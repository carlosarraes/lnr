use serde_json::Value;
use std::process::Command;
fn run(config: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lnr"))
        .args(args)
        .env_remove("LINEAR_API_KEY")
        .env("XDG_CONFIG_HOME", config)
        .output()
        .unwrap()
}
#[test]
fn missing_auth_names_importable_default_without_exposing_key() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("linear")).unwrap();
    std::fs::write(
        d.path().join("linear/credentials.toml"),
        "default = \"mondrio\"\nmondrio = \"secret-never-print\"\n",
    )
    .unwrap();
    let o = run(d.path(), &["issue", "list"]);
    assert_eq!(o.status.code(), Some(3));
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(out.contains("mondrio"), "{out}");
    assert!(!out.contains("secret-never-print"));
}
#[test]
fn auth_status_lists_workspaces_without_requiring_credentials() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("lnr")).unwrap();
    std::fs::write(
        d.path().join("lnr/config.toml"),
        "default = \"one\"\nworkspaces = [\"one\", \"two\"]\n",
    )
    .unwrap();
    let o = run(d.path(), &["auth", "status"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["data"]["default_workspace"], "one");
    assert_eq!(v["data"]["workspaces"], serde_json::json!(["one", "two"]));
}
#[test]
fn oauth_login_is_discoverable_without_existing_auth() {
    let d = tempfile::tempdir().unwrap();
    let o = run(d.path(), &["auth", "login", "--help"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let text = String::from_utf8(o.stdout).unwrap();
    assert!(text.contains("--no-browser"));
    assert!(text.contains("--client-id"));
}

#[test]
fn oauth_no_browser_times_out_without_touching_credentials() {
    let d = tempfile::tempdir().unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    drop(listener);
    let o = run(
        d.path(),
        &[
            "auth",
            "login",
            "--no-browser",
            "--timeout",
            "1",
            "--port",
            &port,
        ],
    );
    assert_eq!(o.status.code(), Some(3));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("timed out")
    );
    assert!(!d.path().join("lnr/config.toml").exists());
    let stderr = String::from_utf8(o.stderr).unwrap();
    assert!(stderr.contains("code_challenge_method=S256"));
    assert!(!stderr.contains("code_verifier="));
}
#[test]
fn sole_workspace_is_the_effective_default_and_default_can_be_changed() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("lnr")).unwrap();
    std::fs::write(d.path().join("lnr/config.toml"), "workspaces = [\"one\"]\n").unwrap();
    let o = run(d.path(), &["auth", "status"]);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["data"]["default_workspace"], "one");
    std::fs::write(
        d.path().join("lnr/config.toml"),
        "workspaces = [\"one\",\"two\"]\n",
    )
    .unwrap();
    let o = run(d.path(), &["auth", "default", "two"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let o = run(d.path(), &["auth", "status"]);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["data"]["default_workspace"], "two");
    let o = run(d.path(), &["auth", "default", "missing"]);
    assert_eq!(o.status.code(), Some(2));
    let config = std::fs::read_to_string(d.path().join("lnr/config.toml")).unwrap();
    assert!(config.contains("default = \"two\""));
}
#[test]
fn inventory_ignores_environment_credentials_even_when_empty() {
    let d = tempfile::tempdir().unwrap();
    for key in ["", "environment-secret"] {
        let o = Command::new(env!("CARGO_BIN_EXE_lnr"))
            .args(["auth", "status"])
            .env("LINEAR_API_KEY", key)
            .env("XDG_CONFIG_HOME", d.path())
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(v["data"]["workspaces"], serde_json::json!([]));
        assert!(
            !String::from_utf8(o.stdout)
                .unwrap()
                .contains("environment-secret")
        );
    }
}
