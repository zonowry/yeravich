use super::*;
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
        .translate("你好，世界。".into(), Language::Chinese, Language::English)
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
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = Url::parse(&format!(
        "http://{}/custom/chat?version=1",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let response = format!(
        "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nLocation: /redirected\r\n\r\n{body}",
        body.len()
    );
    let task = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(5), async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 4096];
            let (headers, body) = loop {
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
                        let body =
                            serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
                        break (headers, body);
                    }
                }
            };
            socket.write_all(response.as_bytes()).await.unwrap();
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
            .translate("你好，世界。\n\"引号\"".into(), source, source.other())
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
        assert_eq!(request["stream"], false);
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
            .translate("text".into(), Language::English, Language::Chinese)
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
            .translate("text".into(), Language::English, Language::Chinese)
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
        .translate("text".into(), Language::English, Language::Chinese)
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
        .translate("text".into(), Language::English, Language::Chinese)
        .await;
    server.abort();
    assert!(matches!(result, Err(Error::Timeout)));
}
