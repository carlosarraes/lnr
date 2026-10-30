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

fn candidates(nodes: serde_json::Value) -> serde_json::Value {
    json!({"data":{"organization":{"name":"Personal","urlKey":"personal"},"projects":page(nodes,false,None)}})
}

#[test]
fn project_names_prefixes_and_slugs_resolve_case_insensitively() {
    for reference in [
        "APPRAISER",
        "apprai",
        "appraiser-tools",
        "abc123",
        "appraiser-tools-abc123",
    ] {
        let s = Server::new(vec![
            (
                200,
                candidates(json!([
                    {"id":"p","name":"Appraiser Tools","slugId":"abc123","url":"https://linear.app/personal/project/appraiser-tools-abc123/overview"},
                    {"id":"other","name":"Billing","slugId":"def456","url":"https://linear.app/personal/project/billing-def456/overview"}
                ])),
            ),
            (
                200,
                json!({"data":{"issues":page(json!([{"id":"i","title":"Result"}]),false,None)}}),
            ),
        ]);
        let o = s.run(&["issue", "list", "--project", reference]);
        assert!(o.status.success(), "{reference}: {:?}", value(&o));
        assert_eq!(value(&o)["data"][0]["id"], "i");
        let r = s.requests.lock().unwrap();
        assert_eq!(r[1]["variables"]["filter"]["project"]["id"]["eq"], "p");
    }
}

#[test]
fn project_exact_name_wins_over_prefix_matches() {
    let s = Server::new(vec![
        (
            200,
            candidates(
                json!([{"id":"p","name":"Appraiser"},{"id":"other","name":"Appraiser Tools"}]),
            ),
        ),
        (
            200,
            json!({"data":{"issueUpdate":{"success":true,"issue":{"id":"i","project":{"id":"p","name":"Appraiser"}}}}}),
        ),
    ]);
    let o = s.run(&["issue", "update", "ENG-1", "--project", "APPRAISER"]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(value(&o)["data"]["project"]["id"], "p");
    assert_eq!(
        s.requests.lock().unwrap()[1]["variables"]["input"]["projectId"],
        "p"
    );
}

#[test]
fn exact_project_name_wins_over_another_projects_generated_slug() {
    let s = Server::new(vec![
        (
            200,
            candidates(json!([
                {"id":"exact","name":"Appraiser-Tools"},
                {"id":"slug","name":"Appraiser Tools"}
            ])),
        ),
        (200, json!({"data":{"issues":page(json!([]),false,None)}})),
    ]);
    let o = s.run(&["issue", "list", "--project", "APPRAISER-TOOLS"]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(
        s.requests.lock().unwrap()[1]["variables"]["filter"]["project"]["id"]["eq"],
        "exact"
    );
}

#[test]
fn project_errors_name_candidates_workspace_and_recovery() {
    for (reference, code, names) in [
        ("app", 5, vec!["Appraiser", "App Store"]),
        ("missing", 4, vec!["Appraiser", "App Store", "Billing"]),
    ] {
        let s = Server::new(vec![(
            200,
            candidates(json!([
                {"id":"a","name":"Appraiser"}, {"id":"b","name":"App Store"}, {"id":"c","name":"Billing"}
            ])),
        )]);
        let o = s.run(&["issue", "list", "--project", reference]);
        assert_eq!(o.status.code(), Some(code));
        let v = value(&o);
        let message = v["error"]["message"].as_str().unwrap();
        for name in names {
            assert!(message.contains(name), "{message}");
        }
        for hint in ["personal", "--workspace SLUG", "LINEAR_API_KEY", "UUID"] {
            assert!(message.contains(hint), "{message}");
        }
        assert_eq!(v["error"]["details"]["workspace"], "personal");
        assert_eq!(s.count(), 1);
    }
}

#[test]
fn project_resolution_paginates_before_accepting_unique_prefix() {
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"organization":{"urlKey":"personal"},"projects":page(json!([{"id":"a","name":"Appraiser"}]),true,Some("next"))}}),
        ),
        (200, candidates(json!([{"id":"b","name":"App Store"}]))),
    ]);
    let o = s.run(&["issue", "list", "--project", "app"]);
    assert_eq!(o.status.code(), Some(5));
    assert!(
        value(&o)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("App Store")
    );
    assert_eq!(s.requests.lock().unwrap()[1]["variables"]["after"], "next");
}

#[test]
fn project_error_candidates_are_capped_at_ten() {
    let nodes: Vec<_> = (0..12)
        .map(|i| json!({"id":format!("p{i}"),"name":format!("Project {i:02}")}))
        .collect();
    let s = Server::new(vec![(200, candidates(json!(nodes)))]);
    let o = s.run(&["issue", "list", "--project", "missing"]);
    assert_eq!(o.status.code(), Some(4));
    let v = value(&o);
    assert_eq!(
        v["error"]["details"]["candidates"]
            .as_array()
            .unwrap()
            .len(),
        10
    );
    assert!(
        !v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Project 10")
    );
}

#[test]
fn incomplete_project_catalog_prevents_a_write() {
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"organization":{"urlKey":"personal"},"projects":page(json!([{"id":"a","name":"Appraiser"}]),true,Some("next"))}}),
        ),
        (200, json!({"errors":[{"message":"Not allowed"}]})),
    ]);
    let o = s.run(&["issue", "update", "ENG-1", "--project", "app"]);
    assert!(!o.status.success());
    assert_eq!(value(&o)["error"]["code"], "api");
    let r = s.requests.lock().unwrap();
    assert_eq!(r.len(), 2);
    assert!(r.iter().all(|r| r["operationName"] == "ResolveProjects"));
}
