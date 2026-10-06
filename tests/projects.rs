mod support;
use serde_json::json;
use support::*;
#[test]
fn project_progress_and_mine_filters() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"projects":page(json!([{"id":"p","name":"P","progress":0.5,"targetDate":null}]),false,None)}}),
    )]);
    let o = s.run(&["project", "list", "--mine"]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(value(&o)["data"][0]["progress"], 0.5);
    assert!(value(&o)["data"][0]["targetDate"].is_null());
    let r = s.requests.lock().unwrap();
    let filter = &r[0]["variables"]["filter"];
    assert_eq!(filter["or"].as_array().unwrap().len(), 3);
    assert_eq!(
        filter["or"][2]["issues"]["some"]["assignee"]["isMe"]["eq"],
        true
    );
    drop(r);
    assert_eq!(s.count(), 1);
}
#[test]
fn default_lists_all_projects() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"projects":page(json!([]),false,None)}}),
    )]);
    let o = s.run(&["project", "list"]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(
        s.requests.lock().unwrap()[0]["variables"]["filter"],
        json!({})
    );
}
#[test]
fn project_issues_are_scoped() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issues":page(json!([]),false,None)}}),
    )]);
    let o = s.run(&[
        "project",
        "issues",
        "00000000-0000-0000-0000-000000000001",
        "--assignee",
        "me",
    ]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(
        s.requests.lock().unwrap()[0]["variables"]["filter"]["project"]["id"]["eq"],
        "00000000-0000-0000-0000-000000000001"
    );
}
