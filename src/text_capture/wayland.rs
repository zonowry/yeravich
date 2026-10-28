use std::{ffi::OsStr, io, process::Stdio, time::Duration};

use tokio::{process::Command, time::timeout};

use super::{Error, Snapshot, usable_text};

pub(super) async fn read(program: &OsStr) -> Snapshot {
    read_with_timeout(program, Duration::from_millis(500)).await
}

async fn read_with_timeout(program: &OsStr, limit: Duration) -> Snapshot {
    let (primary, clipboard) = iced::futures::join!(
        read_selection(program, true, limit),
        read_selection(program, false, limit),
    );
    Snapshot { primary, clipboard }
}

async fn read_selection(program: &OsStr, primary: bool, limit: Duration) -> Result<String, Error> {
    let source = if primary { "primary" } else { "clipboard" };
    let mut command = Command::new(program);
    command
        .args(["--no-newline", "--type", "text"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        // 超时或上层取消任务时，丢弃 future 也结束子进程。
        .kill_on_drop(true);
    if primary {
        command.arg("--primary");
    }
    let output = timeout(limit, command.output())
        .await
        .map_err(|_| {
            eprintln!(
                "[capture] {source} timed out after {} ms",
                limit.as_millis()
            );
            Error::Unavailable
        })?
        .map_err(|error| {
            eprintln!("[capture] {source} reader failed: {:?}", error.kind());
            match error.kind() {
                io::ErrorKind::NotFound => Error::MissingTool,
                _ => Error::Unavailable,
            }
        })?;
    // wl-paste 用非零退出码表示空剪贴板、无文本格式或协议不可用；
    // 不依赖其本地化 stderr 来区分原因。
    if !output.status.success() {
        eprintln!("[capture] {source} reader exited: {}", output.status);
        return Err(Error::NoText);
    }
    let text = String::from_utf8(output.stdout).map_err(|_| Error::Unavailable)?;
    usable_text(Some(text))
}

#[cfg(test)]
mod tests;
