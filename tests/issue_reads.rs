mod support;
use serde_json::json;
use support::*;
#[test]
fn issue_view_returns_details() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issue":{"id":"i","identifier":"ENG-1","title":"Fix","description":"Details","comments":page(json!([]),false,None)}}}),
    )]);
    let o = s.run(&["issue", "view", "ENG-1"]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(value(&o)["data"]["identifier"], "ENG-1");
}
#[test]
fn filters_are_sent_to_server() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issues":page(json!([]),false,None)}}),
    )]);
    let o = s.run(&[
        "issue",
        "list",
        "--assignee",
        "me",
        "--state",
        "started",
        "--search",
        "timeout",
    ]);
    assert!(o.status.success(), "{:?}", value(&o));
    let r = s.requests.lock().unwrap();
    assert_eq!(r[0]["variables"]["filter"]["assignee"]["isMe"]["eq"], true);
    assert_eq!(
        r[0]["variables"]["filter"]["state"]["type"]["eq"],
        "started"
    );
    assert_eq!(
        r[0]["variables"]["filter"]["searchableContent"]["contains"],
        "timeout"
    );
    assert_eq!(r[0]["variables"]["first"], 50);
}
#[test]
fn absent_issue() {
    let s = Server::new(vec![(200, json!({"data":{"issue":null}}))]);
    let o = s.run(&["issue", "view", "ENG-1"]);
    assert_eq!(o.status.code(), Some(4));
}
#[test]
fn ambiguous_project() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"projects":page(json!([{"id":"a","name":"Same"},{"id":"b","name":"Same"}]),false,None)}}),
    )]);
    let o = s.run(&["issue", "list", "--project", "Same"]);
    assert_eq!(o.status.code(), Some(5));
    assert_eq!(s.count(), 1);
}

#[test]
fn title_filter_combines_with_full_text_and_state() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issues":page(json!([{"id":"i","title":"Inconclusive result"}]),false,None)}}),
    )]);
    let o = s.run(&[
        "issue",
        "list",
        "--title",
        "InConclusive",
        "--search",
        "timeout",
        "--state",
        "started",
    ]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(value(&o)["data"][0]["title"], "Inconclusive result");
    let r = s.requests.lock().unwrap();
    assert_eq!(
        r[0]["variables"]["filter"],
        json!({"title":{"containsIgnoreCase":"InConclusive"},"searchableContent":{"contains":"timeout"},"state":{"type":{"eq":"started"}}})
    );
}

#[test]
fn empty_title_filter_is_rejected_without_network() {
    let s = Server::new(vec![]);
    let o = s.run(&["issue", "list", "--title", "  "]);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(s.count(), 0);
}

#[test]
fn get_alias_preserves_view_flags_and_output() {
    for no_comments in [false, true] {
        let reply = json!({"data":{"issue":{"id":"i","identifier":"ENG-1","comments":page(json!([{"id":"c","body":"Hello"}]),false,None)}}});
        let s = Server::new(vec![(200, reply.clone()), (200, reply)]);
        let mut args = vec!["issue", "view", "ENG-1", "--json"];
        if no_comments {
            args.push("--no-comments");
        }
        let view = s.run(&args);
        args[1] = "get";
        let get = s.run(&args);
        assert!(get.status.success(), "{:?}", value(&get));
        assert_eq!(value(&get), value(&view));
        let r = s.requests.lock().unwrap();
        assert_eq!(r[0], r[1]);
    }
}

#[test]
fn unknown_issue_subcommand_uses_json_error_envelope() {
    let s = Server::new(vec![]);
    let o = s.run(&["issue", "unknown", "--json"]);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(value(&o)["schema_version"], 1);
    assert_eq!(value(&o)["error"]["code"], "invalid_input");
    assert_eq!(s.count(), 0);
}
