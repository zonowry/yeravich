use super::Error;

// 限制未完成的一行和 data 缓冲，避免异常响应一直占用内存。
const MAX_EVENT_BYTES: usize = 1024 * 1024;

#[derive(Default)]
pub(super) struct Decoder {
    line: Vec<u8>,
    data: String,
    started: bool,
    skip_lf: bool,
}

impl Decoder {
    pub fn next_event(&mut self, bytes: &mut &[u8]) -> Result<Option<String>, Error> {
        while let Some((&byte, rest)) = bytes.split_first() {
            *bytes = rest;
            if self.skip_lf && byte == b'\n' {
                self.skip_lf = false;
                continue;
            }
            self.skip_lf = byte == b'\r';
            if matches!(byte, b'\r' | b'\n') {
                let event = self.finish_line()?;
                self.line.clear();
                if event.is_some() {
                    return Ok(event);
                }
            } else {
                if self.line.len() + self.data.len() >= MAX_EVENT_BYTES {
                    return Err(Error::InvalidResponse);
                }
                self.line.push(byte);
            }
        }
        Ok(None)
    }

    fn finish_line(&mut self) -> Result<Option<String>, Error> {
        // 先累积字节再解码整行；一个中文字符可能跨越多个 HTTP chunk。
        let mut line = std::str::from_utf8(&self.line).map_err(|_| Error::InvalidResponse)?;
        if !self.started {
            self.started = true;
            line = line.strip_prefix('\u{feff}').unwrap_or(line);
        }
        if line.is_empty() {
            return Ok(if self.data.is_empty() {
                None
            } else {
                self.data.pop(); // SSE 多个 data 字段以换行拼接，分发前去掉最后一个换行。
                Some(std::mem::take(&mut self.data))
            });
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        if field == "data" {
            self.data.push_str(value.strip_prefix(' ').unwrap_or(value));
            self.data.push('\n');
        }
        // 心跳注释及 event/id/retry 等字段不参与翻译正文。
        Ok(None)
    }
}
