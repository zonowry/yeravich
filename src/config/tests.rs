use super::*;

const EXAMPLE: &str = include_str!("../../config.example.toml");

#[test]
fn example_config_uses_the_exact_endpoint_without_authentication() {
    let config = parse(EXAMPLE).unwrap().translation;
    assert_eq!(
        config.endpoint.as_str(),
        "https://index-translate.bilibili.com/v1/chat/completions"
    );
    assert_eq!(config.model, "Index-Translate-35B-A3B");
    assert!(config.authorization.is_none());
}

#[test]
fn optional_key_is_normalized_and_marked_sensitive() {
    for suffix in ["", "api_key = ''", "api_key = '   '"] {
        assert!(
            parse(&format!("{EXAMPLE}\n{suffix}"))
                .unwrap()
                .translation
                .authorization
                .is_none()
        );
    }
    let config = parse(&format!("{EXAMPLE}\napi_key = ' test-secret '"))
        .unwrap()
        .translation;
    let authorization = config.authorization.unwrap();
    assert_eq!(authorization, "Bearer test-secret");
    assert!(authorization.is_sensitive());
    assert!(!format!("{authorization:?}").contains("test-secret"));
}

#[test]
fn rejects_invalid_config_before_sending_any_request() {
    let endpoint = "https://index-translate.bilibili.com/v1/chat/completions";
    for invalid in [
        "[translation]".to_owned(),
        EXAMPLE.replace("endpoint =", "endpont ="),
        EXAMPLE.replace(endpoint, "not a url"),
        EXAMPLE.replace(endpoint, "file:///tmp/translation"),
        EXAMPLE.replace(
            endpoint,
            "https://user:secret@example.com/v1/chat/completions",
        ),
        EXAMPLE.replace(endpoint, "https://example.com/#fragment"),
        EXAMPLE.replace("Index-Translate-35B-A3B", "   "),
        format!("{EXAMPLE}\napi_key = 123"),
        format!("{EXAMPLE}\napi_key = \"bad\\nkey\""),
    ] {
        assert!(parse(&invalid).is_err());
    }
}

#[test]
fn override_path_takes_precedence_over_platform_config_directory() {
    let base = PathBuf::from("platform-config");
    assert_eq!(
        resolve_path(None, Some(base.clone())).unwrap(),
        base.join("yeravich/config.toml")
    );
    assert_eq!(
        resolve_path(Some("custom.toml".into()), None).unwrap(),
        PathBuf::from("custom.toml")
    );
    assert_eq!(
        resolve_path(Some("".into()), Some(base.clone())).unwrap(),
        base.join("yeravich/config.toml")
    );
    assert!(matches!(
        resolve_path(None, None),
        Err(Error::PathUnavailable)
    ));
}

#[tokio::test]
async fn file_errors_include_the_path_without_echoing_contents() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    assert!(matches!(load_from(&path).await, Err(Error::Missing(p)) if p == path));
    assert!(matches!(
        load_from(directory.path()).await,
        Err(Error::Read(_))
    ));

    tokio::fs::write(&path, format!("{EXAMPLE}\napi_key = test-secret"))
        .await
        .unwrap();
    let error = load_from(&path).await.err().unwrap();
    for locale in ["zh-CN", "en"] {
        let message = error.message(locale);
        assert!(message.contains(path.to_str().unwrap()));
        assert!(!message.contains("test-secret"));
    }

    tokio::fs::write(&path, EXAMPLE).await.unwrap();
    assert!(load_from(&path).await.is_ok());
}

#[tokio::test]
async fn first_launch_creates_default_config_in_missing_directories() {
    let directory = tempfile::tempdir().unwrap();
    let base = directory.path().join("new-config-home");
    let config = load_with_paths(None, Some(base.clone())).await.unwrap();
    assert_eq!(
        config.translation.endpoint,
        parse(EXAMPLE).unwrap().translation.endpoint
    );
    assert_eq!(config.translation.model, "Index-Translate-35B-A3B");
    assert!(config.translation.authorization.is_none());
    assert_eq!(config.show_window, Binding::default());
    assert_eq!(
        tokio::fs::read_to_string(base.join("yeravich/config.toml"))
            .await
            .unwrap(),
        EXAMPLE
    );
}

#[tokio::test]
async fn existing_valid_or_invalid_config_is_never_replaced() {
    let directory = tempfile::tempdir().unwrap();
    let base = directory.path().to_owned();
    let path = base.join("yeravich/config.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    for contents in [
        EXAMPLE.replace("Index-Translate-35B-A3B", "custom-model"),
        "invalid config with test-secret".to_owned(),
    ] {
        tokio::fs::write(&path, &contents).await.unwrap();
        let result = load_with_paths(None, Some(base.clone())).await;
        if contents.starts_with("invalid") {
            assert!(matches!(result, Err(Error::Invalid { .. })));
        } else {
            assert_eq!(result.unwrap().translation.model, "custom-model");
        }
        // 也覆盖读取之后其他进程先创建文件的情况。
        create_default(&path).unwrap();
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), contents);
    }
}

#[tokio::test]
async fn explicit_missing_path_is_reported_without_creating_any_config() {
    let directory = tempfile::tempdir().unwrap();
    let base = directory.path().join("default");
    let custom = directory.path().join("custom/config.toml");
    assert!(matches!(
        load_with_paths(Some(custom.clone().into_os_string()), Some(base.clone())).await,
        Err(Error::Missing(path)) if path == custom
    ));
    assert!(!custom.parent().unwrap().exists());
    assert!(!base.exists());

    assert!(
        load_with_paths(Some(OsString::new()), Some(base.clone()))
            .await
            .is_ok()
    );
    assert!(base.join("yeravich/config.toml").is_file());
}

#[tokio::test]
async fn concurrent_first_launches_read_a_complete_config() {
    let directory = tempfile::tempdir().unwrap();
    let base = directory.path().join("new-config-home");
    let (first, second) = tokio::join!(
        load_with_paths(None, Some(base.clone())),
        load_with_paths(None, Some(base.clone()))
    );
    assert_eq!(first.unwrap().translation.model, "Index-Translate-35B-A3B");
    assert_eq!(second.unwrap().translation.model, "Index-Translate-35B-A3B");
    assert_eq!(std::fs::read_dir(base.join("yeravich")).unwrap().count(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn dangling_directory_symlink_reports_creation_error() {
    let directory = tempfile::tempdir().unwrap();
    let base = directory.path().to_owned();
    std::os::unix::fs::symlink(base.join("missing"), base.join("yeravich")).unwrap();
    let error = load_with_paths(None, Some(base.clone()))
        .await
        .err()
        .unwrap();
    assert!(matches!(&error, Error::Create(path) if path == &base.join("yeravich/config.toml")));
    for locale in ["zh-CN", "en"] {
        assert!(error.message(locale).contains(base.to_str().unwrap()));
    }
}

#[tokio::test]
async fn legacy_config_uses_alt_q_without_rewriting_the_file() {
    let directory = tempfile::tempdir().unwrap();
    let base = directory.path().to_owned();
    let path = base.join("yeravich/config.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let legacy = format!(
        "[translation]{}",
        EXAMPLE.split_once("[translation]").unwrap().1
    );
    std::fs::write(&path, &legacy).unwrap();
    let config = load_with_paths(None, Some(base)).await.unwrap();
    assert_eq!(config.show_window, Binding::default());
    assert_eq!(std::fs::read_to_string(path).unwrap(), legacy);
}

#[test]
fn shortcut_config_accepts_custom_bindings_and_rejects_typos() {
    let config = parse(&EXAMPLE.replace("Alt+Q", "Ctrl+Shift+T")).unwrap();
    assert_eq!(config.show_window, Binding::parse("Ctrl+Shift+T").unwrap());
    assert_eq!(
        parse(&EXAMPLE.replace("Alt+Q", "Alt+")).err(),
        Some("shortcuts.show_window")
    );
    assert!(parse(&EXAMPLE.replace("show_window =", "show_widnow =")).is_err());
    assert!(parse(&EXAMPLE.replace("show_window = \"Alt+Q\"", "show_window = 42")).is_err());
    assert_eq!(
        parse(&EXAMPLE.replace("show_window = \"Alt+Q\"", ""))
            .unwrap()
            .show_window,
        Binding::default()
    );
}
