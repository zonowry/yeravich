use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

use super::*;

struct Request {
    path: String,
    authorization: Option<String>,
    body: serde_json::Value,
}

async fn server(status: u16, body: String, delay: Duration) -> (String, JoinHandle<Request>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        let (header_end, length) = loop {
            let mut buffer = [0; 1024];
            let count = socket.read(&mut buffer).await.unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buffer[..count]);
            if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                break (end + 4, length);
            }
        };
        while bytes.len() < header_end + length {
            let mut buffer = [0; 1024];
            let count = socket.read(&mut buffer).await.unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buffer[..count]);
        }
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let path = headers
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .to_owned();
        let authorization = headers.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("authorization")
                .then(|| value.trim().to_owned())
        });
        let body_json = serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap();
        tokio::time::sleep(delay).await;
        let response = format!(
            "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nLocation: /must-not-follow\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        Request {
            path,
            authorization,
            body: body_json,
        }
    });
    (address, task)
}

fn success() -> String {
    r#"{"choices":[{"message":{"content":"你好"}}]}"#.into()
}

#[test]
fn accepts_base_urls_custom_prefixes_and_full_endpoints() {
    for (address, endpoint) in [
        (
            "http://localhost:8080",
            "http://localhost:8080/v1/chat/completions",
        ),
        (
            "https://example.com/v1/",
            "https://example.com/v1/chat/completions",
        ),
        (
            "https://example.com/custom/api",
            "https://example.com/custom/api/chat/completions",
        ),
        (
            "https://example.com/v1/chat/completions/",
            "https://example.com/v1/chat/completions",
        ),
    ] {
        assert_eq!(
            ChatSettings::new(address, " model ").unwrap().endpoint(),
            endpoint
        );
    }
    for address in [
        "file:///tmp/file",
        "ftp://example.com",
        "not a URL",
        "https://user@example.com",
        "https://example.com?key=value",
        "https://example.com/#fragment",
    ] {
        assert_eq!(
            ChatSettings::new(address, "model").unwrap_err(),
            ChatError::InvalidAddress
        );
    }
    assert_eq!(
        ChatSettings::new("https://example.com", " \n ").unwrap_err(),
        ChatError::MissingModel
    );
}

#[tokio::test]
async fn connection_check_sends_selected_model_and_bearer_authentication() {
    let (address, server) = server(200, success(), Duration::ZERO).await;
    let settings = ChatSettings::new(&format!("{address}/prefix/v1/"), "custom-model").unwrap();
    let key = uuid::Uuid::new_v4().to_string();
    ChatClient::new()
        .unwrap()
        .test_connection(&settings, Some(SecretValue::new(key.as_bytes().to_vec())))
        .await
        .unwrap();
    let request = server.await.unwrap();
    assert_eq!(request.path, "/prefix/v1/chat/completions");
    let correct_authorization =
        request.authorization.as_deref() == Some(format!("Bearer {key}").as_str());
    assert!(correct_authorization, "authorization header did not match");
    assert_eq!(request.body["model"], "custom-model");
    assert_eq!(request.body["stream"], false);
    assert_eq!(request.body["messages"][0]["role"], "user");
}

#[tokio::test]
async fn translation_uses_same_adapter_and_preserves_source_text() {
    let (address, server) = server(200, success(), Duration::ZERO).await;
    let settings = ChatSettings::new(&address, "model").unwrap();
    let translation = ChatClient::new()
        .unwrap()
        .translate(
            TranslationRequest {
                text: "hello".into(),
                languages: crate::LanguagePair::default(),
                profile: settings.profile(None),
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(translation.text, "你好");
    assert_eq!(translation.source, "hello");
    let request = server.await.unwrap();
    assert!(request.authorization.is_none());
    assert_eq!(request.body["messages"][1]["content"], "hello");
    assert!(
        request.body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("zh-CN")
    );
}

#[tokio::test]
async fn http_errors_are_classified_without_exposing_remote_bodies() {
    for (status, expected) in [
        (401, ChatError::Authentication(401)),
        (403, ChatError::Authentication(403)),
        (404, ChatError::NotFound),
        (429, ChatError::RateLimited),
        (500, ChatError::Http(500)),
        (307, ChatError::Http(307)),
    ] {
        let marker = uuid::Uuid::new_v4().to_string();
        let (address, server) = server(status, marker.clone(), Duration::ZERO).await;
        let settings = ChatSettings::new(&address, "model").unwrap();
        let error = ChatClient::new()
            .unwrap()
            .test_connection(&settings, None)
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains(&marker));
        assert!(!format!("{error:?}").contains(&marker));
        server.await.unwrap();
    }
}

#[tokio::test]
async fn rejects_success_responses_that_are_not_chat_text() {
    for body in [
        "not json",
        r#"{"choices":[]}"#,
        r#"{"choices":[{"message":{"content":null}}]}"#,
        r#"{"choices":[{"message":{"content":" "}}]}"#,
    ] {
        let (address, server) = server(200, body.into(), Duration::ZERO).await;
        let settings = ChatSettings::new(&address, "model").unwrap();
        assert_eq!(
            ChatClient::new()
                .unwrap()
                .test_connection(&settings, None)
                .await
                .unwrap_err(),
            ChatError::InvalidResponse
        );
        server.await.unwrap();
    }
}

#[tokio::test]
async fn reports_timeouts_and_limits_response_size() {
    let (address, server) = server(200, success(), Duration::from_millis(200)).await;
    let settings = ChatSettings::new(&address, "model").unwrap();
    let error = ChatClient::new()
        .unwrap()
        .complete(&settings, None, Vec::new(), Duration::from_millis(30))
        .await
        .unwrap_err();
    assert_eq!(error, ChatError::Timeout);
    server.await.unwrap();

    let (address, server) =
        self::server(200, "x".repeat(MAX_RESPONSE_BYTES + 1), Duration::ZERO).await;
    let settings = ChatSettings::new(&address, "model").unwrap();
    assert_eq!(
        ChatClient::new()
            .unwrap()
            .test_connection(&settings, None)
            .await
            .unwrap_err(),
        ChatError::ResponseTooLarge
    );
    server.await.unwrap();
}
