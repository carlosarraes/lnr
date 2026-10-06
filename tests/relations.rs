mod support;
use serde_json::json;
use support::*;
#[test]
fn blocks_preserves_direction() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issueRelationCreate":{"success":true,"issueRelation":{"id":"r"}}}}),
    )]);
    let o = s.run(&[
        "issue",
        "relation",
        "add",
        "00000000-0000-0000-0000-000000000001",
        "--blocks",
        "00000000-0000-0000-0000-000000000002",
    ]);
    assert!(o.status.success(), "{:?}", value(&o));
    let r = s.requests.lock().unwrap();
    assert_eq!(r[0]["variables"]["input"]["type"], "blocks");
    assert_eq!(
        r[0]["variables"]["input"]["issueId"],
        "00000000-0000-0000-0000-000000000001"
    );
}
#[test]
fn relation_target_required() {
    let s = Server::new(vec![]);
    let o = s.run(&["issue", "relation", "add", "ENG-1"]);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(s.count(), 0);
}
