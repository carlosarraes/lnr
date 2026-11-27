mod support;
use serde_json::{Value, json};
use support::*;

fn candidate(identifier: &str, description: &str, blockers: Value) -> Value {
    let incoming: Vec<_> = blockers
        .as_array()
        .unwrap()
        .iter()
        .map(|b| json!({"type":b[0],"issue":{"id":b[1],"identifier":b[1],"state":{"type":b[2]}}}))
        .collect();
    json!({"id":identifier,"identifier":identifier,"title":identifier,"description":description,
        "state":{"id":"s","name":"Todo","type":"unstarted"},
        "inverseRelations":page(json!(incoming),false,None)})
}
fn ready_ids(v: &Value) -> Vec<&str> {
    v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["identifier"].as_str().unwrap())
        .collect()
}

#[test]
fn ready_drops_issues_with_unfinished_relations_or_gates() {
    let issues = json!([
        candidate("ENG-10", "No gates here", json!([])),
        candidate("ENG-11", "", json!([["blocks", "ENG-1", "started"]])),
        candidate("ENG-12", "Build after ABC-1 lands", json!([])),
        candidate(
            "ENG-13",
            "build AFTER abc-2, then after ENG-3.",
            json!([
                ["blocks", "ENG-2", "completed"],
                ["related", "ENG-9", "started"]
            ])
        ),
        candidate("ENG-14", "Only after ABC-9", json!([])),
        candidate("ENG-15", "", json!([["blocks", "ENG-5", "canceled"]])),
        candidate("ENG-16", "Thereafter ABC-1 is not a gate", json!([])),
    ]);
    let gates = json!([
        {"id":"g1","identifier":"ABC-1","state":{"type":"started"}},
        {"id":"g2","identifier":"ABC-2","state":{"type":"completed"}},
        {"id":"g3","identifier":"ENG-3","state":{"type":"completed"}}
    ]);
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"issues":page(issues,true,Some("more"))}}),
        ),
        (200, json!({"data":{"issues":page(gates,false,None)}})),
    ]);
    let o = s.run(&["issue", "ready"]);
    assert!(o.status.success(), "{:?}", value(&o));
    let v = value(&o);
    assert_eq!(ready_ids(&v), ["ENG-10", "ENG-13", "ENG-16"]);
    assert!(v["data"][0].get("inverse_relations").is_none());
    assert_eq!(v["meta"]["page"]["end_cursor"], "more");
    let blocked: Vec<_> = v["meta"]["blocked"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| (b["identifier"].as_str().unwrap(), b["blockers"][0].clone()))
        .collect();
    assert_eq!(blocked.len(), 4);
    assert_eq!(
        blocked[0],
        (
            "ENG-11",
            json!({"identifier":"ENG-1","source":"relation","state":"started"})
        )
    );
    assert_eq!(
        blocked[1],
        (
            "ENG-12",
            json!({"identifier":"ABC-1","source":"description","state":"started"})
        )
    );
    assert_eq!(blocked[2].1["state"], Value::Null);
    assert_eq!(blocked[3].1["state"], "canceled");
    let r = s.requests.lock().unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(r[0]["operationName"], "ReadyIssues");
    let filter = &r[0]["variables"]["filter"];
    assert_eq!(filter["assignee"]["isMe"]["eq"], true);
    assert_eq!(filter["state"]["type"]["eq"], "unstarted");
    assert_eq!(r[0]["variables"]["first"], 50);
    assert_eq!(
        r[1]["variables"]["filter"],
        json!({"or":[
            {"team":{"key":{"eq":"ABC"}},"number":{"in":[1,2,9]}},
            {"team":{"key":{"eq":"ENG"}},"number":{"in":[3]}}
        ]})
    );
}

#[test]
fn ready_without_gates_needs_one_request_and_resolves_team_names() {
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"organization":{"urlKey":"acme"},"teams":page(json!([{"id":"t1","key":"MON","name":"Engineering team"}]),false,None)}}),
        ),
        (
            200,
            json!({"data":{"issues":page(json!([candidate("MON-1","",json!([]))]),false,None)}}),
        ),
    ]);
    let o = s.run(&[
        "issue",
        "ready",
        "--assignee",
        "me",
        "--team",
        "Engineering",
        "--state",
        "started",
        "--all",
    ]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(ready_ids(&value(&o)), ["MON-1"]);
    let r = s.requests.lock().unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(r[1]["variables"]["filter"]["team"]["id"]["eq"], "t1");
    assert_eq!(
        r[1]["variables"]["filter"]["state"]["type"]["eq"],
        "started"
    );
}

#[test]
fn ready_treats_unverifiable_relation_pages_as_blocking() {
    let mut issue = candidate("ENG-1", "", json!([]));
    issue["inverseRelations"] = page(json!([]), true, Some("next"));
    let s = Server::new(vec![(
        200,
        json!({"data":{"issues":page(json!([issue]),false,None)}}),
    )]);
    let o = s.run(&["issue", "ready"]);
    assert!(o.status.success(), "{:?}", value(&o));
    let v = value(&o);
    assert_eq!(ready_ids(&v), Vec::<&str>::new());
    assert_eq!(v["meta"]["blocked"][0]["blockers"][0]["unverified"], true);
}

#[test]
fn ready_state_names_require_team() {
    let s = Server::new(vec![]);
    let o = s.run(&["issue", "ready", "--state", "In Review"]);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(s.count(), 0);
}
