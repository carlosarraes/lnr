mod support;
use serde_json::json;
use support::*;
#[test]
fn graphql_partial_error_is_not_success() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"viewer":{"id":"u"}},"errors":[{"message":"denied"}]}),
    )]);
    let o = s.run(&["auth", "status"]);
    assert_eq!(o.status.code(), Some(8));
    assert_eq!(value(&o)["data"]["viewer"]["id"], "u");
    assert_eq!(s.count(), 1);
}
#[test]
fn query_retries_then_succeeds() {
    let s = Server::new(vec![
        (503, json!({})),
        (
            200,
            json!({"data":{"viewer":{"id":"u","name":"Me"},"organization":{"urlKey":"test"}}}),
        ),
    ]);
    let o = s.run(&["auth", "status"]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(s.count(), 2);
}
#[test]
fn mutation_never_retries() {
    let s = Server::new(vec![(503, json!({}))]);
    let o = s.run_input(
        &["api", "--query-file", "-"],
        Some("mutation { issueDelete(id: \"x\") { success } }"),
        true,
    );
    assert_eq!(o.status.code(), Some(8));
    assert_eq!(value(&o)["error"]["code"], "uncertain_mutation");
    assert_eq!(s.count(), 1);
}
#[test]
fn exhausted_rate_limit() {
    let s = Server::new(vec![(429, json!({})), (429, json!({})), (429, json!({}))]);
    let o = s.run(&["auth", "status"]);
    assert_eq!(o.status.code(), Some(6));
    assert_eq!(s.count(), 3);
}
#[test]
fn secrets_are_redacted() {
    let s = Server::new(vec![(
        200,
        json!({"errors":[{"message":"bad secret-test-key"}]}),
    )]);
    let o = s.run(&["auth", "status"]);
    assert!(!String::from_utf8_lossy(&o.stdout).contains("secret-test-key"));
    assert!(!String::from_utf8_lossy(&o.stderr).contains("secret-test-key"));
}
#[test]
fn sends_only_selected_operation() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"viewer":{"id":"u"},"organization":{"urlKey":"test"}}}),
    )]);
    let o = s.run(&["auth", "status"]);
    assert!(o.status.success());
    let r = s.requests.lock().unwrap();
    let q = r[0]["query"].as_str().unwrap();
    assert!(!q.contains("mutation"));
    assert!(!q.contains("IssueFields"));
}
#[test]
fn rate_limit_reset_beyond_budget_returns_without_retry() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let s = Server::with_headers(
        vec![(
            400,
            json!({"errors":[{"message":"limited","extensions":{"code":"RATELIMITED"}}]}),
        )],
        format!(
            "X-RateLimit-Requests-Remaining: 0\r\nX-RateLimit-Requests-Reset: {}\r\n",
            now + 120_000
        ),
    );
    let o = s.run(&["auth", "status"]);
    assert_eq!(o.status.code(), Some(6));
    assert_eq!(s.count(), 1);
    assert!(
        value(&o)["error"]["details"]["retry_after_ms"]
            .as_u64()
            .unwrap()
            > 30_000
    );
}
#[test]
fn partial_data_cannot_echo_credentials() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"echo":"secret-test-key"},"errors":[{"message":"bad"}]}),
    )]);
    let o = s.run(&["auth", "status"]);
    assert!(!String::from_utf8_lossy(&o.stdout).contains("secret-test-key"));
}
#[test]
fn oversized_response_fails() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"large":"x".repeat(16*1024*1024)}}),
    )]);
    let o = s.run(&["auth", "status"]);
    assert_eq!(o.status.code(), Some(8));
    assert_eq!(s.count(), 1);
    assert!(
        value(&o)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("16 MiB")
    );
}
