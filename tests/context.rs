mod support;
use serde_json::json;
use support::*;
#[test]
fn context_is_bounded_with_independent_cursors() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issue":{"id":"i","identifier":"ENG-1","title":"Fix","parent":null,"project":null,"children":page(json!([]),false,None),"comments":page(json!([{"id":"c","body":"hello"}]),true,Some("c2")),"relations":page(json!([]),false,None),"inverseRelations":page(json!([]),true,Some("r2"))}}}),
    )]);
    let o = s.run(&["issue", "context", "ENG-1", "--comments-after", "c1"]);
    assert!(o.status.success(), "{:?}", value(&o));
    let v = value(&o);
    assert_eq!(v["meta"]["collections"]["comments"]["has_more"], true);
    assert_eq!(v["meta"]["collections"]["children"]["complete"], true);
    assert_eq!(
        v["meta"]["collections"]["inverse_relations"]["end_cursor"],
        "r2"
    );
    assert_eq!(s.count(), 1);
    let r = s.requests.lock().unwrap();
    assert_eq!(r[0]["variables"]["limit"], 20);
    assert_eq!(r[0]["variables"]["commentsAfter"], "c1");
    assert!(r[0]["variables"]["childrenAfter"].is_null());
}
#[test]
fn context_partial_error() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issue":{"id":"i","comments":null}},"errors":[{"message":"comments inaccessible"}]}),
    )]);
    let o = s.run(&["issue", "context", "ENG-1"]);
    assert_eq!(o.status.code(), Some(8));
    assert_eq!(value(&o)["data"]["id"], "i");
}
