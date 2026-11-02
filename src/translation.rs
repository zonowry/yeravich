use std::{fmt, time::Duration};

use iced::futures::{SinkExt, Stream, channel::mpsc};
use reqwest::{Client, header::AUTHORIZATION, header::CONTENT_TYPE};
use rust_i18n::t;
use serde::{Deserialize, Serialize};

use crate::config::{self, TranslationConfig};
use crate::i18n::{Langs, Language};

mod sse;

#[cfg(test)]
mod tests;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub enum Event {
    Delta(String),
    Finished(Result<(), Error>),
}

#[derive(Debug, Clone)]
pub enum Error {
    Config(config::Error),
    Client,
    Timeout,
    Network,
    Http(u16),
    InvalidResponse,
    EmptyResponse,
    IncompleteResponse,
}

impl Error {
    pub fn message(&self, locale: &str) -> String {
        let key = match self {
            Self::Config(error) => return error.message(locale),
            Self::Http(status) => {
                return t!(
                    Langs::TRANSLATION_HTTP_ERROR,
                    locale = locale,
                    status = status
                )
                .into_owned();
            }
            Self::Client => Langs::TRANSLATION_CLIENT_ERROR,
            Self::Timeout => Langs::TRANSLATION_TIMEOUT,
            Self::Network => Langs::TRANSLATION_NETWORK_ERROR,
            Self::InvalidResponse => Langs::TRANSLATION_INVALID_RESPONSE,
            Self::EmptyResponse => Langs::TRANSLATION_EMPTY_RESPONSE,
            Self::IncompleteResponse => Langs::TRANSLATION_INCOMPLETE_RESPONSE,
        };
        t!(key, locale = locale).into_owned()
    }

    fn from_request(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::Timeout
        } else if error.is_decode() {
            Self::InvalidResponse
        } else {
            Self::Network
        }
    }
}

#[derive(Clone)]
pub struct Service {
    client: Client,
    config: TranslationConfig,
}

impl fmt::Debug for Service {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 消息需要 Debug，但配置、URL 查询参数和密钥不应进入调试输出。
        formatter.debug_struct("Service").finish_non_exhaustive()
    }
}

impl Service {
    #[cfg(test)]
    pub async fn load() -> Result<Self, Error> {
        let config = config::load().await.map_err(Error::Config)?;
        Self::new(config.translation)
    }

    pub fn new(config: TranslationConfig) -> Result<Self, Error> {
        let client = client_builder().build().map_err(|_| Error::Client)?;
        Ok(Self { client, config })
    }

    pub fn translate(
        self,
        text: String,
        source: Language,
        target: Language,
    ) -> impl Stream<Item = Event> {
        iced::stream::channel(16, async move |mut output| {
            let result = self.request(text, source, target, &mut output).await;
            // 片段与结束消息共用有界通道，避免错误抢在已生成的片段之前到达。
            let _ = output.send(Event::Finished(result)).await;
        })
    }

    async fn request(
        &self,
        text: String,
        source: Language,
        target: Language,
        output: &mut mpsc::Sender<Event>,
    ) -> Result<(), Error> {
        let prompt = format!(
            "请将以下文本从{source}翻译为{target}，直接输出翻译结果，不要进行任何解释。\n\n{text}"
        );
        let body = ChatRequest {
            model: &self.config.model,
            messages: [ChatMessage {
                role: "user",
                content: &prompt,
            }],
            stream: true,
        };
        let mut request = self.client.post(self.config.endpoint.clone()).json(&body);
        if let Some(authorization) = &self.config.authorization {
            request = request.header(AUTHORIZATION, authorization.clone());
        }

        let mut response = request.send().await.map_err(Error::from_request)?;
        if !response.status().is_success() {
            // 不展示服务端原始错误正文，以免回显密钥或原文。
            return Err(Error::Http(response.status().as_u16()));
        }
        let is_stream = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("text/event-stream"));
        if is_stream {
            let mut decoder = sse::Decoder::default();
            let mut has_text = false;
            while let Some(chunk) = response.chunk().await.map_err(Error::from_request)? {
                let mut bytes = chunk.as_ref();
                while let Some(data) = decoder.next_event(&mut bytes)? {
                    if data.trim() == "[DONE]" {
                        return if has_text {
                            Ok(())
                        } else {
                            Err(Error::EmptyResponse)
                        };
                    }
                    if data.trim().is_empty() {
                        continue;
                    }
                    let chunk: ChatChunk =
                        serde_json::from_str(&data).map_err(|_| Error::InvalidResponse)?;
                    // usage 帧的 choices 为空；只显示第一个候选的正文。
                    if let Some(choice) = chunk.choices.into_iter().find(|choice| choice.index == 0)
                    {
                        if let Some(content) = choice.delta.content.filter(|text| !text.is_empty())
                        {
                            has_text |= !content.trim().is_empty();
                            let _ = output.send(Event::Delta(content)).await;
                        }
                        if choice
                            .finish_reason
                            .as_deref()
                            .is_some_and(|reason| reason != "stop")
                        {
                            return Err(Error::IncompleteResponse);
                        }
                    }
                }
            }
            // EOF 不是成功标记：即使已有部分译文，也不能把截断的流当成完整结果。
            return Err(Error::IncompleteResponse);
        }

        // 某些兼容服务忽略 stream，仍返回完整 JSON；直接使用这次响应。
        let response: ChatResponse = response.json().await.map_err(Error::from_request)?;
        let content = response
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or(Error::EmptyResponse)?;
        let _ = output.send(Event::Delta(content)).await;
        Ok(())
    }
}

fn client_builder() -> reqwest::ClientBuilder {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(REQUEST_TIMEOUT)
        // 只向用户配置的地址发送文本，重定向通过 HTTP 错误提示修正地址。
        .redirect(reqwest::redirect::Policy::none())
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: [ChatMessage<'a>; 1],
    stream: bool,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ChatChunk {
    choices: Vec<ChunkChoice>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    index: usize,
    delta: ResponseMessage,
    finish_reason: Option<String>,
}
