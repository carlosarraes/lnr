mod support;
use support::*;
#[test]
fn missing_credentials() {
    let s = Server::new(vec![]);
    let o = s.run_input(&["auth", "status"], None, false);
    assert_eq!(o.status.code(), Some(3));
    assert_eq!(s.count(), 0);
}
#[test]
fn explicit_workspace_conflicts_with_environment() {
    let s = Server::new(vec![]);
    let o = s.run(&["--workspace", "other", "auth", "status"]);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(s.count(), 0);
}
#[test]
fn import_requires_explicit_workspace() {
    let s = Server::new(vec![]);
    let o = s.run_input(&["auth", "import-linear"], None, false);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(s.count(), 0);
}
#[test]
fn auth_status_uses_normalized_fields() {
    let s = Server::new(vec![(
        200,
        serde_json::json!({"data":{"viewer":{"id":"u"},"organization":{"id":"o","name":"Org","urlKey":"workspace"}}}),
    )]);
    let o = s.run(&["auth", "status"]);
    assert!(o.status.success());
    assert_eq!(value(&o)["data"]["organization"]["url_key"], "workspace");
}
