mod support;
use serde_json::json;
use support::*;
#[test]
fn empty_update_rejected_without_network() {
    let s = Server::new(vec![]);
    let o = s.run(&["issue", "update", "ENG-1"]);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(s.count(), 0);
}
#[test]
fn create_preserves_multiline_body() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issueCreate":{"success":true,"issue":{"id":"i","identifier":"ENG-1"}}}}),
    )]);
    let o = s.run_input(
        &[
            "issue",
            "create",
            "--team",
            "00000000-0000-0000-0000-000000000001",
            "--title",
            "Fix",
            "--description-file",
            "-",
        ],
        Some("Olá\n\nbody\n"),
        true,
    );
    assert!(o.status.success(), "{:?}", value(&o));
    let r = s.requests.lock().unwrap();
    assert_eq!(r[0]["variables"]["input"]["description"], "Olá\n\nbody\n");
    assert!(r[0]["variables"]["input"].get("assigneeId").is_none());
}
#[test]
fn clear_is_distinct_from_omission() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issueUpdate":{"success":true,"issue":{"id":"i"}}}}),
    )]);
    let o = s.run(&["issue", "update", "ENG-1", "--clear-project"]);
    assert!(o.status.success(), "{:?}", value(&o));
    let r = s.requests.lock().unwrap();
    let input = &r[0]["variables"]["input"];
    assert!(input.get("projectId").unwrap().is_null());
    assert!(input.get("parentId").is_none());
    drop(r);
    assert_eq!(s.count(), 1);
}
#[test]
fn unsuccessful_mutation_is_error() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issueUpdate":{"success":false,"issue":null}}}),
    )]);
    let o = s.run(&["issue", "update", "ENG-1", "--title", "Fix"]);
    assert_eq!(o.status.code(), Some(8));
    assert_eq!(s.count(), 1);
    assert_eq!(value(&o)["error"]["code"], "mutation_rejected");
}
#[test]
fn conflicting_clear_is_rejected() {
    let s = Server::new(vec![]);
    let o = s.run(&[
        "issue",
        "update",
        "ENG-1",
        "--clear-project",
        "--project",
        "A",
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(s.count(), 0);
}
#[test]
fn create_returns_branch_name_and_creator() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issueCreate":{"success":true,"issue":{"id":"i","identifier":"ENG-1","branchName":"carlos/eng-1-fix","creator":{"id":"u","name":"Carlos"}}}}}),
    )]);
    let o = s.run(&[
        "issue",
        "create",
        "--team",
        "00000000-0000-0000-0000-000000000001",
        "--title",
        "Fix",
    ]);
    assert!(o.status.success(), "{:?}", value(&o));
    let data = &value(&o)["data"];
    assert_eq!(data["branch_name"], "carlos/eng-1-fix");
    assert_eq!(data["creator"]["name"], "Carlos");
    let r = s.requests.lock().unwrap();
    let query = r[0]["query"].as_str().unwrap();
    assert!(query.contains("branchName") && query.contains("creator"));
}
