mod support;
use serde_json::json;
use support::*;
#[test]
fn view_fetches_all_comments_and_returns_newest_last() {
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"issue":{"id":"i","identifier":"ENG-1","comments":page(json!([{"id":"new","body":"latest discussion","createdAt":"2026-10-06T12:00:00Z"}]),true,Some("next"))}}}),
        ),
        (
            200,
            json!({"data":{"issue":{"comments":page(json!([{"id":"old","body":"earlier discussion","createdAt":"2026-10-05T12:00:00Z"}]),false,None)}}}),
        ),
    ]);
    let o = s.run(&["issue", "view", "ENG-1"]);
    assert!(o.status.success(), "{:?}", value(&o));
    let v = value(&o);
    assert_eq!(v["data"]["comments"][0]["id"], "old");
    assert_eq!(v["data"]["comments"][1]["body"], "latest discussion");
    assert_eq!(v["meta"]["collections"]["comments"]["complete"], true);
    let requests = s.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1]["variables"]["after"], "next");
}
#[test]
fn no_comments_skips_comment_requests_and_omits_the_field() {
    let s = Server::new(vec![(200, json!({"data":{"issue":{"id":"i"}}}))]);
    let o = s.run(&["issue", "view", "ENG-1", "--no-comments"]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert!(value(&o)["data"].get("comments").is_none());
    assert_eq!(s.count(), 1);
}
#[test]
fn partial_comment_page_keeps_issue_and_available_discussion() {
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"issue":{"id":"i","comments":page(json!([{"id":"a","createdAt":"2026-10-06T12:00:00Z"}]),true,Some("next"))}}}),
        ),
        (
            200,
            json!({"data":{"issue":{"comments":null}},"errors":[{"message":"comments unavailable"}]}),
        ),
    ]);
    let o = s.run(&["issue", "view", "ENG-1"]);
    assert!(!o.status.success());
    assert_eq!(value(&o)["data"]["id"], "i");
    assert_eq!(value(&o)["data"]["comments"][0]["id"], "a");
    assert_eq!(value(&o)["meta"]["complete"], false);
}
