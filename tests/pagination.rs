mod support;
use serde_json::json;
use support::*;
#[test]
fn all_pages() {
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"issues":page(json!([{"id":"a"}]),true,Some("next"))}}),
        ),
        (
            200,
            json!({"data":{"issues":page(json!([{"id":"b"}]),false,None)}}),
        ),
    ]);
    let o = s.run(&["issue", "list", "--all"]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(value(&o)["data"].as_array().unwrap().len(), 2);
    assert_eq!(s.requests.lock().unwrap()[1]["variables"]["after"], "next");
}
#[test]
fn repeated_cursor_fails() {
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"issues":page(json!([]),true,Some("x"))}}),
        ),
        (
            200,
            json!({"data":{"issues":page(json!([]),true,Some("x"))}}),
        ),
    ]);
    let o = s.run(&["issue", "list", "--all"]);
    assert_eq!(o.status.code(), Some(8));
    assert_eq!(s.count(), 2);
}
#[test]
fn missing_cursor_fails() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issues":page(json!([]),true,None)}}),
    )]);
    let o = s.run(&["issue", "list"]);
    assert_eq!(o.status.code(), Some(8));
}
#[test]
fn later_failure_preserves_items() {
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"issues":page(json!([{"id":"a"}]),true,Some("x"))}}),
        ),
        (200, json!({"errors":[{"message":"denied"}]})),
    ]);
    let o = s.run(&["issue", "list", "--all"]);
    assert_eq!(o.status.code(), Some(8));
    assert_eq!(value(&o)["data"][0]["id"], "a");
}
