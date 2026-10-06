mod support;
use serde_json::json;
use support::*;

#[test]
fn later_partial_issue_page_keeps_both_pages() {
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"issues":page(json!([{"id":"a"}]),true,Some("next"))}}),
        ),
        (
            200,
            json!({"data":{"issues":page(json!([{"id":"b"}]),false,None)},"errors":[{"message":"partial"}]}),
        ),
    ]);
    let o = s.run(&["issue", "list", "--all"]);
    assert_eq!(o.status.code(), Some(8));
    assert_eq!(value(&o)["data"].as_array().unwrap().len(), 2);
}
#[test]
fn malformed_comment_page_keeps_prior_and_current_items() {
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"issue":{"comments":page(json!([{"id":"a","body":"a"}]),true,Some("next"))}}}),
        ),
        (
            200,
            json!({"data":{"issue":{"comments":page(json!([{"id":"b","body":"b"}]),true,None)}}}),
        ),
    ]);
    let o = s.run(&["issue", "comment", "list", "ENG-1", "--all"]);
    assert_eq!(o.status.code(), Some(8));
    assert_eq!(value(&o)["data"].as_array().unwrap().len(), 2);
}
#[test]
fn partial_relations_keep_available_direction() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issue":{"relations":page(json!([{"id":"r","relatedIssue":{"id":"i"}}]),false,None),"inverseRelations":null}},"errors":[{"message":"incoming unavailable"}]}),
    )]);
    let o = s.run(&["issue", "relation", "list", "ENG-1"]);
    assert_eq!(o.status.code(), Some(8));
    assert_eq!(value(&o)["data"]["relations"][0]["id"], "r");
    assert_eq!(
        value(&o)["meta"]["collections"]["inverse_relations"]["complete"],
        false
    );
}
#[test]
fn partial_context_has_the_same_shape_and_collection_metadata() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issue":{"id":"i","children":page(json!([]),false,None),"comments":page(json!([{"id":"c","body":"text","createdAt":"date"}]),false,None),"relations":null,"inverseRelations":page(json!([]),false,None)}},"errors":[{"message":"relation unavailable"}]}),
    )]);
    let o = s.run(&["issue", "context", "ENG-1"]);
    let v = value(&o);
    assert_eq!(o.status.code(), Some(8));
    assert_eq!(v["data"]["id"], "i");
    assert_eq!(v["data"]["comments"][0]["created_at"], "date");
    assert_eq!(v["meta"]["collections"]["relations"]["complete"], false);
}
#[test]
fn project_list_uses_project_dto_keys() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"projects":page(json!([{"id":"p","targetDate":"2026-12-01"}]),false,None)}}),
    )]);
    let o = s.run(&["project", "list"]);
    assert!(o.status.success());
    assert_eq!(value(&o)["data"][0]["target_date"], "2026-12-01");
    assert!(value(&o)["data"][0].get("targetDate").is_none());
}
#[test]
fn workspace_labels_are_in_scope() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issueLabels":page(json!([]),false,None)}}),
    )]);
    let o = s.run(&[
        "label",
        "list",
        "--team",
        "00000000-0000-0000-0000-000000000001",
    ]);
    assert!(o.status.success());
    let requests = s.requests.lock().unwrap();
    let filter = &requests[0]["variables"]["filter"];
    assert_eq!(filter["or"][1]["team"]["null"], true);
}
#[test]
fn state_uuid_needs_no_team_lookup() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issues":page(json!([]),false,None)}}),
    )]);
    let o = s.run(&[
        "issue",
        "list",
        "--state",
        "00000000-0000-0000-0000-000000000001",
    ]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(s.count(), 1);
}
#[test]
fn discovery_describes_context_and_conditional_requirements() {
    let s = Server::new(vec![]);
    let o = s.run(&["schema"]);
    let v = value(&o);
    let cmds = v["data"]["commands"].as_array().unwrap();
    let context = cmds
        .iter()
        .find(|c| c["path"] == "lnr issue context")
        .unwrap();
    assert_eq!(context["output"], "issue_context");
    assert!(v["data"]["outputs"]["issue_context"]["properties"]["meta"].is_object());
    let state = cmds.iter().find(|c| c["path"] == "lnr state list").unwrap();
    let team = state["arguments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == "team")
        .unwrap();
    assert_eq!(team["required"], true);
    let rel = cmds
        .iter()
        .find(|c| c["path"] == "lnr issue relation add")
        .unwrap();
    assert_eq!(
        rel["constraints"]["exactly_one"],
        json!(["blocks", "related", "duplicate_of"])
    );
}
#[cfg(target_os = "linux")]
#[test]
fn linux_import_reads_upstream_secret_service_attributes() {
    use std::{os::unix::fs::PermissionsExt, process::Command};
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("linear")).unwrap();
    std::fs::write(
        dir.path().join("linear/credentials.toml"),
        "workspaces = ['test']\n",
    )
    .unwrap();
    let tool = dir.path().join("secret-tool");
    std::fs::write(&tool,"#!/bin/sh\n[ \"$*\" = 'lookup service linear-cli account test' ] || exit 1\nprintf fixture-key\n").unwrap();
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o700)).unwrap();
    let s = Server::new(vec![(
        200,
        json!({"data":{"viewer":{"id":"u"},"organization":{"urlKey":"different"}}}),
    )]);
    let o = Command::new(env!("CARGO_BIN_EXE_lnr"))
        .args(["auth", "import-linear", "--workspace", "test", "--json"])
        .env_remove("LINEAR_API_KEY")
        .env("XDG_CONFIG_HOME", dir.path())
        .env("LNR_API_URL", &s.url)
        .env(
            "PATH",
            format!(
                "{}:{}",
                dir.path().display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .output()
        .unwrap();
    assert_eq!(s.count(), 1, "{}", String::from_utf8_lossy(&o.stdout));
    assert_eq!(o.status.code(), Some(3));
    assert!(!dir.path().join("lnr/config.toml").exists());
}
