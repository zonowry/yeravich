use std::{fs, os::unix::fs::PermissionsExt};

use super::*;

fn reader(script: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wl-paste");
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    (directory, path)
}

#[tokio::test]
async fn both_sources_are_read_without_changing_either() {
    let (_directory, program) = reader(
        r#"test "$1" = --no-newline && test "$2" = --type && test "$3" = text || exit 2
if [ "$4" = --primary ]; then printf '  中文\nselected\n'; else printf 'clipboard'; fi"#,
    );
    let snapshot = read_with_timeout(program.as_os_str(), Duration::from_secs(5)).await;
    assert_eq!(snapshot.primary, Ok("  中文\nselected\n".into()));
    assert_eq!(snapshot.clipboard, Ok("clipboard".into()));
}

#[tokio::test]
async fn absent_empty_unsupported_or_invalid_selection_falls_back_to_clipboard() {
    for primary in ["exit 1", "printf ' \n\t'", "printf '\\377'"] {
        let (_directory, program) = reader(&format!(
            "if [ \"$4\" = --primary ]; then {primary}; else printf 'clipboard'; fi"
        ));
        assert_eq!(
            read_with_timeout(program.as_os_str(), Duration::from_secs(5))
                .await
                .clipboard,
            Ok("clipboard".into())
        );
    }
}

#[tokio::test]
async fn empty_clipboard_missing_tool_and_invalid_text_are_reported() {
    let (directory, program) = reader("exit 1");
    assert_eq!(
        read_with_timeout(program.as_os_str(), Duration::from_secs(5))
            .await
            .clipboard,
        Err(Error::NoText)
    );
    assert_eq!(
        read_with_timeout(
            directory.path().join("missing").as_os_str(),
            Duration::from_secs(5)
        )
        .await
        .clipboard,
        Err(Error::MissingTool)
    );
    let (_directory, program) = reader("printf '\\377'");
    assert_eq!(
        read_with_timeout(program.as_os_str(), Duration::from_secs(5))
            .await
            .clipboard,
        Err(Error::Unavailable)
    );
}

#[tokio::test]
async fn a_stalled_selection_is_killed_and_clipboard_is_still_tried() {
    let (_directory, program) = reader(
        r#"if [ "$4" = --primary ]; then
printf '%s' "$$" > "$0.pid"
exec sleep 30
else printf 'fallback'; fi"#,
    );
    assert_eq!(
        timeout(
            Duration::from_secs(5),
            read_with_timeout(program.as_os_str(), Duration::from_secs(1))
        )
        .await
        .unwrap()
        .clipboard,
        Ok("fallback".into())
    );
    let pid = fs::read_to_string(program.with_extension("pid")).unwrap();
    // Tokio 的子进程回收在事件循环中完成，等待它确认退出。
    timeout(Duration::from_secs(5), async {
        while std::path::Path::new(&format!("/proc/{pid}")).exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn cancelling_the_reader_also_ends_its_child_process() {
    let (_directory, program) = reader("printf '%s' \"$$\" > \"$0.$4.pid\"\nexec sleep 30");
    let pid_files = [
        program.with_file_name("wl-paste.--primary.pid"),
        program.with_file_name("wl-paste..pid"),
    ];
    let task = tokio::spawn(async move {
        read_with_timeout(program.as_os_str(), Duration::from_secs(5)).await
    });
    timeout(Duration::from_secs(5), async {
        while pid_files
            .iter()
            .any(|path| !fs::read_to_string(path).is_ok_and(|pid| !pid.is_empty()))
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    for pid_file in pid_files {
        let pid = fs::read_to_string(pid_file).unwrap();
        timeout(Duration::from_secs(5), async {
            while std::path::Path::new(&format!("/proc/{pid}")).exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
}
