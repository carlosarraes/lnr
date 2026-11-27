mod support;
use serde_json::{Value, json};
use support::*;

fn teams(nodes: Value) -> Value {
    json!({"data":{"organization":{"name":"Acme","urlKey":"acme"},"teams":page(nodes,false,None)}})
}
fn workspace_teams() -> Value {
    teams(json!([
        {"id":"t1","key":"MON","name":"Engineering team"},
        {"id":"t2","key":"PROD","name":"Product team"}
    ]))
}
fn issues() -> Value {
    json!({"data":{"issues":page(json!([]),false,None)}})
}

#[test]
fn team_keys_names_and_unique_prefixes_resolve_case_insensitively() {
    for (reference, id) in [
        ("MON", "t1"),
        ("mon", "t1"),
        ("Engineering team", "t1"),
        ("engineering TEAM", "t1"),
        ("Engineering", "t1"),
        ("prod", "t2"),
        ("Product", "t2"),
    ] {
        let s = Server::new(vec![(200, workspace_teams()), (200, issues())]);
        let o = s.run(&["issue", "list", "--team", reference]);
        assert!(o.status.success(), "{reference}: {:?}", value(&o));
        let r = s.requests.lock().unwrap();
        assert_eq!(r[0]["operationName"], "ResolveTeams");
        assert_eq!(
            r[1]["variables"]["filter"]["team"]["id"]["eq"], id,
            "{reference}"
        );
    }
}

#[test]
fn exact_team_key_wins_over_another_teams_name_prefix() {
    let s = Server::new(vec![
        (
            200,
            teams(json!([
                {"id":"key","key":"OPS","name":"Operations"},
                {"id":"prefix","key":"OPX","name":"Ops tooling"}
            ])),
        ),
        (200, issues()),
    ]);
    let o = s.run(&["issue", "list", "--team", "ops"]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(
        s.requests.lock().unwrap()[1]["variables"]["filter"]["team"]["id"]["eq"],
        "key"
    );
}

#[test]
fn shared_team_prefix_is_ambiguous_with_candidates() {
    let s = Server::new(vec![(200, workspace_teams())]);
    let o = s.run(&["issue", "list", "--team", "team"]);
    assert_eq!(o.status.code(), Some(5));
    let v = value(&o);
    assert_eq!(v["error"]["code"], "ambiguous");
    let candidates = v["error"]["details"]["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 2);
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Multiple teams match")
    );
    assert_eq!(s.count(), 1);
}

#[test]
fn team_miss_lists_candidates_without_blaming_the_workspace() {
    let s = Server::new(vec![(200, workspace_teams())]);
    let o = s.run(&["issue", "list", "--team", "Design"]);
    assert_eq!(o.status.code(), Some(4));
    let v = value(&o);
    let message = v["error"]["message"].as_str().unwrap();
    assert!(
        message.starts_with("No team matches \"Design\""),
        "{message}"
    );
    assert!(
        message.contains("MON (Engineering team), PROD (Product team)"),
        "{message}"
    );
    assert!(message.contains("lnr team list"), "{message}");
    assert!(!message.contains("isn't in it"), "{message}");
    assert!(!message.contains("credential"), "{message}");
    assert_eq!(v["error"]["details"]["workspace"], "acme");
    assert!(v["error"]["details"].get("hint").is_none());
    assert_eq!(s.count(), 1);
}

#[test]
fn team_miss_with_no_visible_teams_keeps_the_workspace_hint() {
    let s = Server::new(vec![
        (200, teams(json!([]))),
        (
            200,
            json!({"data":{"viewer":{"id":"u"},"organization":{"id":"o","urlKey":"acme"}}}),
        ),
    ]);
    let o = s.run(&["issue", "list", "--team", "MON"]);
    assert_eq!(o.status.code(), Some(4));
    let message = value(&o)["error"]["message"].as_str().unwrap().to_owned();
    assert!(message.contains("workspace: acme"), "{message}");
    assert_eq!(s.count(), 2);
}

#[test]
fn team_resolution_reads_every_page_and_applies_to_catalog_flags() {
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"organization":{"urlKey":"acme"},"teams":page(json!([{"id":"t1","key":"MON","name":"Engineering team"}]),true,Some("next"))}}),
        ),
        (
            200,
            teams(json!([{"id":"t2","key":"ENGX","name":"Engineering extras"}])),
        ),
        (
            200,
            json!({"data":{"workflowStates":page(json!([]),false,None)}}),
        ),
    ]);
    let o = s.run(&["state", "list", "--team", "engineering team"]);
    assert!(o.status.success(), "{:?}", value(&o));
    let r = s.requests.lock().unwrap();
    assert_eq!(r[1]["variables"]["after"], "next");
    assert_eq!(r[2]["variables"]["filter"]["team"]["id"]["eq"], "t1");
}

#[test]
fn team_uuid_skips_resolution() {
    let s = Server::new(vec![(200, issues())]);
    let o = s.run(&[
        "issue",
        "list",
        "--team",
        "00000000-0000-0000-0000-000000000001",
    ]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(s.count(), 1);
}

#[test]
fn text_errors_strip_terminal_escapes_from_team_candidates() {
    let s = Server::new(vec![(
        200,
        teams(json!([{"id":"t1","key":"MON","name":"Eng\u{1b}]8;;http://evil\u{7}\u{1b}[2J"}])),
    )]);
    let o = s.run(&["issue", "list", "--team", "Design", "--output", "text"]);
    assert_eq!(o.status.code(), Some(4));
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(stderr.contains("MON (Eng"), "{stderr}");
    assert!(
        !stderr.contains('\u{1b}') && !stderr.contains('\u{7}'),
        "{stderr:?}"
    );
}
