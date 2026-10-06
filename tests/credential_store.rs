#![cfg(unix)]
mod support;
use serde_json::json;
use std::{os::unix::fs::PermissionsExt, process::Command};
use support::*;
fn run(server: &Server, config: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lnr"))
        .args(args)
        .env_remove("LINEAR_API_KEY")
        .env("XDG_CONFIG_HOME", config)
        .env("LNR_CREDENTIAL_STORE", "file")
        .env("LNR_API_URL", &server.url)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap()
}
fn legacy(config: &std::path::Path) {
    std::fs::create_dir_all(config.join("linear")).unwrap();
    std::fs::write(
        config.join("linear/credentials.toml"),
        "default = 'mondrio'\nmondrio = 'fixture-private-key'\n",
    )
    .unwrap();
}
fn status() -> serde_json::Value {
    json!({"data":{"viewer":{"id":"u"},"organization":{"urlKey":"mondrio"}}})
}
#[test]
fn headless_import_persists_private_file_and_authenticates_next_process() {
    let d = tempfile::tempdir().unwrap();
    legacy(d.path());
    let s = Server::new(vec![(200, status()), (200, status())]);
    let o = run(
        &s,
        d.path(),
        &["auth", "import-linear", "--workspace", "mondrio"],
    );
    assert!(o.status.success(), "{:?}", value(&o));
    let path = d.path().join("lnr/credentials.json");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(d.path().join("lnr"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let o = run(&s, d.path(), &["auth", "status", "--check"]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert!(!String::from_utf8_lossy(&o.stdout).contains("fixture-private-key"));
    assert_eq!(s.count(), 2);
}
#[test]
fn missing_file_credential_has_an_executable_recovery_hint() {
    let d = tempfile::tempdir().unwrap();
    legacy(d.path());
    std::fs::create_dir(d.path().join("lnr")).unwrap();
    std::fs::write(
        d.path().join("lnr/config.toml"),
        "default = 'mondrio'\nworkspaces = ['mondrio']\n",
    )
    .unwrap();
    let s = Server::new(vec![]);
    let o = run(&s, d.path(), &["issue", "list"]);
    assert_eq!(o.status.code(), Some(3));
    assert!(
        value(&o)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("lnr auth import-linear --workspace mondrio")
    );
    assert_eq!(s.count(), 0);
}
#[test]
fn insecure_existing_file_is_rejected_without_exposing_credentials() {
    let d = tempfile::tempdir().unwrap();
    legacy(d.path());
    let s = Server::new(vec![(200, status())]);
    let o = run(
        &s,
        d.path(),
        &["auth", "import-linear", "--workspace", "mondrio"],
    );
    assert!(o.status.success(), "{:?}", value(&o));
    let path = d.path().join("lnr/credentials.json");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let o = run(&s, d.path(), &["auth", "status", "--check"]);
    assert_eq!(o.status.code(), Some(9));
    assert!(
        value(&o)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("0600")
    );
    assert!(!String::from_utf8_lossy(&o.stdout).contains("fixture-private-key"));
}
#[test]
fn concurrent_agent_imports_preserve_every_workspace() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("linear")).unwrap();
    let source = (0..15)
        .map(|i| format!("workspace{i} = 'fixture-key-{i}'\n"))
        .collect::<String>();
    std::fs::write(d.path().join("linear/credentials.toml"), source).unwrap();
    std::thread::scope(|scope| {
        let mut handles = vec![];
        for i in 0..15 {
            let root = d.path();
            handles.push(scope.spawn(move || {
                let workspace = format!("workspace{i}");
                let s = Server::new(vec![(
                    200,
                    json!({"data":{"viewer":{"id":"u"},"organization":{"urlKey":workspace}}}),
                )]);
                let o = run(
                    &s,
                    root,
                    &["auth", "import-linear", "--workspace", &workspace],
                );
                assert!(o.status.success(), "{:?}", value(&o));
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
    });
    let credentials: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("lnr/credentials.json")).unwrap())
            .unwrap();
    assert_eq!(credentials.as_object().unwrap().len(), 15);
    let config: toml::Value =
        toml::from_str(&std::fs::read_to_string(d.path().join("lnr/config.toml")).unwrap())
            .unwrap();
    assert_eq!(config["workspaces"].as_array().unwrap().len(), 15);
}
#[test]
fn symlink_credential_file_is_not_followed_or_overwritten() {
    let d = tempfile::tempdir().unwrap();
    legacy(d.path());
    std::fs::create_dir(d.path().join("lnr")).unwrap();
    let external = d.path().join("external");
    std::fs::write(&external, "do not replace").unwrap();
    std::os::unix::fs::symlink(&external, d.path().join("lnr/credentials.json")).unwrap();
    let s = Server::new(vec![(200, status())]);
    let o = run(
        &s,
        d.path(),
        &["auth", "import-linear", "--workspace", "mondrio"],
    );
    assert_eq!(o.status.code(), Some(9));
    assert_eq!(std::fs::read_to_string(external).unwrap(), "do not replace");
}
