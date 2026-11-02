use super::*;
use iced::futures::{StreamExt, pin_mut};
use reqwest::{Url, header::HeaderValue};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

#[tokio::test]
#[ignore = "显式运行才会向配置的真实服务发送示例文本"]
async fn configured_service_smoke_test() {
    let service = Service::load().await.unwrap();
    let translated = service
        .collect_translation("你好，世界。".into(), Language::Chinese, Language::English)
        .await
        .unwrap();
    assert!(!translated.trim().is_empty());
    println!("示例译文：{translated}");
}

fn service(endpoint: Url, key: Option<&str>) -> Service {
    Service {
        client: client_builder().no_proxy().build().unwrap(),
        config: TranslationConfig {
            endpoint,
            model: "test-model".into(),
            authorization: key.map(|key| HeaderValue::from_str(&format!("Bearer {key}")).unwrap()),
        },
    }
}

// 本地单次 HTTP 服务：捕获实际请求并发送受控响应，测试不依赖外部接口。
async fn mock_server(status: u16, body: &str) -> (Url, JoinHandle<(String, Value)>) {
    mock_response(
        status,
        "application/json",
        vec![body.as_bytes().to_vec()],
        None,
    )
    .await
}

async fn mock_response(
    status: u16,
    content_type: &str,
    parts: Vec<Vec<u8>>,
    mut after_first: Option<iced::futures::channel::oneshot::Receiver<()>>,
) -> (Url, JoinHandle<(String, Value)>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = Url::parse(&format!(
        "http://{}/custom/chat?version=1",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let response = format!(
        "HTTP/1.1 {status} Test\r\nContent-Type: {content_type}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\nLocation: /redirected\r\n\r\n"
    );
    let task = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(5), async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let (headers, body) = read_request(&mut socket).await;
            socket.write_all(response.as_bytes()).await.unwrap();
            for (index, part) in parts.into_iter().enumerate() {
                if !part.is_empty() {
                    socket
                        .write_all(format!("{:x}\r\n", part.len()).as_bytes())
                        .await
                        .unwrap();
                    socket.write_all(&part).await.unwrap();
                    socket.write_all(b"\r\n").await.unwrap();
                }
                if index == 0
                    && let Some(acknowledged) = after_first.take()
                {
                    // 服务端等客户端确认已显示首段后才发送后续内容，避免靠 sleep 猜时序。
                    acknowledged.await.unwrap();
                }
            }
            socket.write_all(b"0\r\n\r\n").await.unwrap();
            (headers, body)
        })
        .await
        .unwrap()
    });
    (endpoint, task)
}

#[tokio::test]
async fn sends_chat_request_with_optional_auth_and_both_directions() {
    for (key, source) in [
        (None, Language::Chinese),
        (Some("test-secret"), Language::English),
    ] {
        let translated = "  Hello, world.\n第二行  ";
        let body = json!({"choices": [{"message": {"content": translated}}]}).to_string();
        let (endpoint, server) = mock_server(200, &body).await;
        let service = service(endpoint, key);
        let result = service
            .collect_translation("你好，世界。\n\"引号\"".into(), source, source.other())
            .await
            .unwrap();
        assert_eq!(result, translated);
        let (headers, request) = server.await.unwrap();
        assert!(headers.starts_with("POST /custom/chat?version=1 HTTP/1.1"));
        assert!(
            headers
                .to_lowercase()
                .contains("content-type: application/json")
        );
        assert_eq!(
            headers
                .to_lowercase()
                .contains("authorization: bearer test-secret"),
            key.is_some()
        );
        assert_eq!(request["model"], "test-model");
        assert_eq!(request["stream"], true);
        assert_eq!(request["messages"].as_array().unwrap().len(), 1);
        assert_eq!(request["messages"][0]["role"], "user");
        let prompt = request["messages"][0]["content"].as_str().unwrap();
        assert!(prompt.contains(&format!("从{source}翻译为{}", source.other())));
        assert!(prompt.ends_with("你好，世界。\n\"引号\""));
        assert!(!format!("{service:?}").contains("test-secret"));
    }
}

#[tokio::test]
async fn reports_http_errors_without_following_redirects_or_echoing_bodies() {
    for status in [302, 401, 403, 429, 500] {
        let (endpoint, server) = mock_server(status, "test-secret").await;
        let error = service(endpoint, None)
            .collect_translation("text".into(), Language::English, Language::Chinese)
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Http(code) if code == status));
        assert!(!error.message("en").contains("test-secret"));
        server.await.unwrap();
    }
}

#[tokio::test]
async fn rejects_malformed_or_empty_success_responses() {
    for (body, empty) in [
        ("not json", false),
        ("{}", false),
        (r#"{"choices":[{"message":{"content":42}}]}"#, false),
        (r#"{"choices":[]}"#, true),
        (r#"{"choices":[{"message":{"content":null}}]}"#, true),
        (r#"{"choices":[{"message":{}}]}"#, true),
        (r#"{"choices":[{"message":{"content":"  \n "}}]}"#, true),
    ] {
        let (endpoint, server) = mock_server(200, body).await;
        let error = service(endpoint, None)
            .collect_translation("text".into(), Language::English, Language::Chinese)
            .await
            .unwrap_err();
        assert!(if empty {
            matches!(error, Error::EmptyResponse)
        } else {
            matches!(error, Error::InvalidResponse)
        });
        server.await.unwrap();
    }
}

#[tokio::test]
async fn reports_connection_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    drop(listener);
    let result = service(endpoint, None)
        .collect_translation("text".into(), Language::English, Language::Chinese)
        .await;
    assert!(matches!(result, Err(Error::Network)));
}

#[tokio::test]
async fn timeout_covers_reading_the_response_body() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        let _ = socket.read(&mut request).await.unwrap();
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n")
            .await
            .unwrap();
        std::future::pending::<()>().await;
    });
    let mut service = service(endpoint, None);
    service.client = client_builder()
        .no_proxy()
        .timeout(Duration::from_millis(100))
        .build()
        .unwrap();
    let result = service
        .collect_translation("text".into(), Language::English, Language::Chinese)
        .await;
    server.abort();
    assert!(matches!(result, Err(Error::Timeout)));
}

impl Service {
    async fn collect_translation(
        &self,
        text: String,
        source: Language,
        target: Language,
    ) -> Result<String, Error> {
        let stream = self.clone().translate(text, source, target);
        pin_mut!(stream);
        let mut translated = String::new();
        let mut finished = false;
        while let Some(event) = stream.next().await {
            assert!(!finished, "no events should follow completion");
            match event {
                Event::Delta(delta) => translated.push_str(&delta),
                Event::Finished(result) => {
                    result?;
                    finished = true;
                }
            }
        }
        assert!(finished, "stream must report completion");
        Ok(translated)
    }
}

fn delta_event(content: &str) -> String {
    format!(
        "data: {}\n\n",
        json!({"choices": [{"index": 0, "delta": {"content": content}}]})
    )
}

#[tokio::test]
async fn streams_content_before_the_response_finishes() {
    let first = delta_event("  你好");
    let rest = format!("{}data: [DONE]\n\n", delta_event("，世界\n第二行  "));
    let (acknowledge, acknowledged) = iced::futures::channel::oneshot::channel();
    let (endpoint, server) = mock_response(
        200,
        "text/event-stream; charset=utf-8",
        vec![first.into_bytes(), rest.into_bytes()],
        Some(acknowledged),
    )
    .await;
    let stream =
        service(endpoint, None).translate("Hello".into(), Language::English, Language::Chinese);
    pin_mut!(stream);
    let first = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .unwrap();
    assert!(matches!(first, Some(Event::Delta(ref text)) if text == "  你好"));
    acknowledge.send(()).unwrap();
    assert!(
        matches!(stream.next().await, Some(Event::Delta(ref text)) if text == "，世界\n第二行  ")
    );
    assert!(matches!(stream.next().await, Some(Event::Finished(Ok(())))));
    assert!(stream.next().await.is_none());
    let (_, request) = server.await.unwrap();
    assert_eq!(request["stream"], true);
}

#[tokio::test]
async fn parses_fragmented_unicode_and_ignores_role_usage_and_other_choices() {
    let body = format!(
        "\u{feff}: heartbeat\r\n\r\ndata: {{\"choices\":[{{\"index\":0,\"delta\":{{\"role\":\"assistant\",\"content\":null}}}}]}}\r\n\r\n{}data: {{\"choices\":[{{\"index\":1,\"delta\":{{\"content\":\"wrong\"}}}},{{\"index\":0,\"delta\":{{\"content\":\"世界🌍\"}}}}]}}\n\ndata: {{\"choices\":[{{\"index\":0,\"delta\":{{}},\"finish_reason\":\"stop\"}}]}}\n\ndata: {{\"choices\":[],\"usage\":{{\"total_tokens\":5}}}}\n\ndata: [DONE]\n\n",
        delta_event("你好，")
    );
    let (endpoint, server) = mock_response(
        200,
        "TEXT/EVENT-STREAM",
        body.bytes().map(|byte| vec![byte]).collect(),
        None,
    )
    .await;
    assert_eq!(
        service(endpoint, None)
            .collect_translation("text".into(), Language::English, Language::Chinese)
            .await
            .unwrap(),
        "你好，世界🌍"
    );
    server.await.unwrap();
}

#[tokio::test]
async fn stream_errors_follow_partial_content_and_never_echo_server_details() {
    for (tail, incomplete) in [
        ("", true),
        ("data: [DONE]", true), // 未经空行结束的事件不能分发。
        ("data: {broken json}\n\n", false),
        (
            "event: error\ndata: {\"error\":{\"message\":\"test-secret\"}}\n\n",
            false,
        ),
        (
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":42}}]}\n\n",
            false,
        ),
        (
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"length\"}]}\n\ndata: [DONE]\n\n",
            true,
        ),
        (
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"content_filter\"}]}\n\ndata: [DONE]\n\n",
            true,
        ),
    ] {
        let body = format!("{}{tail}", delta_event("partial"));
        let (endpoint, server) =
            mock_response(200, "text/event-stream", vec![body.into_bytes()], None).await;
        let stream =
            service(endpoint, None).translate("text".into(), Language::English, Language::Chinese);
        pin_mut!(stream);
        assert!(matches!(stream.next().await, Some(Event::Delta(ref text)) if text == "partial"));
        let Some(Event::Finished(Err(error))) = stream.next().await else {
            panic!("expected a stream failure")
        };
        assert!(if incomplete {
            matches!(error, Error::IncompleteResponse)
        } else {
            matches!(error, Error::InvalidResponse)
        });
        for locale in ["en", "zh-CN"] {
            assert!(!error.message(locale).contains("test-secret"));
        }
        assert!(stream.next().await.is_none());
        server.await.unwrap();
    }
}

#[tokio::test]
async fn done_without_translation_is_empty_and_trailing_data_is_ignored() {
    for content in ["", " \n "] {
        let body = format!(
            "{}data: [DONE]\n\n{}",
            delta_event(content),
            delta_event("too late")
        );
        let (endpoint, server) =
            mock_response(200, "text/event-stream", vec![body.into_bytes()], None).await;
        assert!(matches!(
            service(endpoint, None)
                .collect_translation("text".into(), Language::English, Language::Chinese)
                .await,
            Err(Error::EmptyResponse)
        ));
        server.await.unwrap();
    }
    let body = format!(
        "{}data: [DONE]\n\n{}",
        delta_event("valid"),
        delta_event("too late")
    );
    let (endpoint, server) =
        mock_response(200, "text/event-stream", vec![body.into_bytes()], None).await;
    assert_eq!(
        service(endpoint, None)
            .collect_translation("text".into(), Language::English, Language::Chinese)
            .await
            .unwrap(),
        "valid"
    );
    server.await.unwrap();
}

#[test]
fn sse_decoder_handles_every_split_point_and_line_ending() {
    for ending in ["\n", "\r\n", "\r"] {
        let body = format!(
            "\u{feff}: heartbeat{ending}{ending}event: message{ending}id: 1{ending}data: {{\"choices\":{ending}data: []}}{ending}{ending}data: 中文🌍{ending}{ending}data: [DONE]{ending}{ending}"
        );
        for split in 0..=body.len() {
            let mut decoder = sse::Decoder::default();
            let mut events = Vec::new();
            for mut bytes in [&body.as_bytes()[..split], &body.as_bytes()[split..]] {
                while let Some(event) = decoder.next_event(&mut bytes).unwrap() {
                    events.push(event);
                }
            }
            assert_eq!(events, ["{\"choices\":\n[]}", "中文🌍", "[DONE]"]);
        }
    }
}

#[test]
fn sse_decoder_rejects_invalid_utf8_and_unbounded_events() {
    for bytes in [
        b"data: \xff\n\n".to_vec(),
        vec![b'x'; 1024 * 1024 + 1],
        "data: x\n".repeat(600_000).into_bytes(),
    ] {
        let mut decoder = sse::Decoder::default();
        assert!(matches!(
            decoder.next_event(&mut bytes.as_slice()),
            Err(Error::InvalidResponse)
        ));
    }
}

#[tokio::test]
async fn stream_timeout_keeps_partial_content_and_finishes_with_an_error() {
    let (acknowledge, acknowledged) = iced::futures::channel::oneshot::channel();
    let (endpoint, server) = mock_response(
        200,
        "text/event-stream",
        vec![delta_event("partial").into_bytes()],
        Some(acknowledged),
    )
    .await;
    let mut service = service(endpoint, None);
    service.client = client_builder()
        .no_proxy()
        .timeout(Duration::from_millis(100))
        .build()
        .unwrap();
    let stream = service.translate("text".into(), Language::English, Language::Chinese);
    pin_mut!(stream);
    assert!(matches!(stream.next().await, Some(Event::Delta(ref text)) if text == "partial"));
    assert!(matches!(
        stream.next().await,
        Some(Event::Finished(Err(Error::Timeout)))
    ));
    assert!(stream.next().await.is_none());
    server.abort();
    drop(acknowledge);
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> (String, Value) {
    let mut bytes = Vec::new();
    let mut chunk = [0; 4096];
    loop {
        let size = socket.read(&mut chunk).await.unwrap();
        assert!(size > 0, "request ended before body was read");
        bytes.extend_from_slice(&chunk[..size]);
        if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            let headers = String::from_utf8(bytes[..end].to_vec()).unwrap();
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().unwrap())
                })
                .unwrap();
            if bytes.len() >= end + 4 + length {
                let body = serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
                return (headers, body);
            }
        }
    }
}

#[tokio::test]
async fn cancelling_a_translation_drops_the_response_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = read_request(&mut socket).await;
        let part = delta_event("partial");
        socket.write_all(format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{part}\r\n", part.len()
        ).as_bytes()).await.unwrap();
        let mut byte = [0];
        tokio::time::timeout(Duration::from_secs(2), socket.read(&mut byte))
            .await
            .unwrap()
            .unwrap()
    });
    let stream =
        service(endpoint, None).translate("text".into(), Language::English, Language::Chinese);
    // 与 Task::abortable 相同的 Future/Stream 取消边界；不另起脱离句柄的网络任务。
    let (stream, abort) = iced::futures::stream::abortable(stream);
    {
        pin_mut!(stream);
        assert!(matches!(stream.next().await, Some(Event::Delta(_))));
        abort.abort();
        assert!(stream.next().await.is_none());
    }
    assert_eq!(
        server.await.unwrap(),
        0,
        "server should observe a closed connection"
    );
}
