use std::process::Command;
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lnr"))
        .env_remove("LINEAR_API_KEY")
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn help_without_credentials() {
    let o = run(&["--help"]);
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).contains("issue"));
}
#[test]
fn invalid_args_json() {
    let o = run(&["issue", "view", "--json"]);
    assert_eq!(o.status.code(), Some(2));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["error"]["code"], "invalid_input");
}
#[test]
fn output_conflict() {
    let o = run(&["--json", "--output", "text", "auth", "status"]);
    assert_eq!(o.status.code(), Some(2));
}
#[test]
fn argument_named_text_does_not_disable_json() {
    let o = run(&[
        "issue",
        "update",
        "ENG-1",
        "--title",
        "text",
        "--priority",
        "9",
    ]);
    assert_eq!(o.status.code(), Some(2));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["error"]["code"], "invalid_input");
}
