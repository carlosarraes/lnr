mod support;
use serde_json::{Value, json};
use support::*;

fn entity_not_found() -> Value {
    json!({"data":null,"errors":[{"message":"Entity not found: Issue","extensions":{"code":"INVALID_INPUT","userError":true}}]})
}
fn organization() -> Value {
    json!({"data":{"viewer":{"id":"u","name":"Carlos"},"organization":{"id":"o","name":"Carraes","urlKey":"carraes"}}})
}
fn output_text(o: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}
fn issue() -> Value {
    json!({"data":{"issue":{
        "id":"i","identifier":"ENG-7","title":"Fix login","description":"Steps to reproduce\nline two",
        "state":{"id":"s","name":"In Progress","type":"started"},
        "assignee":null,
        "labels":{"nodes":[{"id":"l1","name":"bug"},{"id":"l2","name":"backend"}]},
        "comments":page(json!([{"id":"c","body":"looks good","createdAt":"2026-10-06T12:00:00Z","user":{"id":"u","name":"Ana"}}]),false,None)
    }}})
}

#[test]
fn entity_not_found_exits_4_naming_workspace_and_env_source_without_the_key() {
    let s = Server::new(vec![(200, entity_not_found()), (200, organization())]);
    let o = s.run(&["issue", "view", "MON-13351"]);
    assert_eq!(o.status.code(), Some(4), "{}", output_text(&o));
    let v = value(&o);
    assert_eq!(v["error"]["code"], "not_found");
    let message = v["error"]["message"].as_str().unwrap();
    assert!(message.contains("LINEAR_API_KEY"), "{message}");
    assert!(
        message.contains(
            "this key is for workspace carraes (Carraes); MON-13351 isn't in it. Use the card text from your brief instead."
        ),
        "{message}"
    );
    assert_eq!(v["error"]["details"]["workspace"], "carraes (Carraes)");
    assert!(!output_text(&o).contains("secret-test-key"));
    let requests = s.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]["query"]
            .as_str()
            .unwrap()
            .contains("organization")
    );
}

#[test]
fn not_found_text_error_carries_hint_on_stderr() {
    let s = Server::new(vec![(400, entity_not_found()), (200, organization())]);
    let o = s.run(&["issue", "view", "MON-13351", "--output", "text"]);
    assert_eq!(o.status.code(), Some(4));
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(stderr.starts_with("not_found: "), "{stderr}");
    assert!(stderr.contains("MON-13351 isn't in it"), "{stderr}");
    assert!(!output_text(&o).contains("secret-test-key"));
}

#[test]
fn failed_organization_lookup_still_exits_4_with_a_hint() {
    let s = Server::new(vec![
        (200, entity_not_found()),
        (500, json!({"errors":[{"message":"boom"}]})),
    ]);
    let o = s.run(&["issue", "view", "MON-13351"]);
    assert_eq!(o.status.code(), Some(4));
    let v = value(&o);
    let message = v["error"]["message"].as_str().unwrap();
    assert!(message.contains("organization lookup failed"), "{message}");
    assert!(message.contains("MON-13351 isn't in it"), "{message}");
    assert_eq!(v["error"]["details"]["workspace"], Value::Null);
}

#[cfg(unix)]
#[test]
fn stored_credential_source_names_the_config_file() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("linear")).unwrap();
    std::fs::write(
        d.path().join("linear/credentials.toml"),
        "default = 'carraes'\ncarraes = 'fixture-private-key'\n",
    )
    .unwrap();
    let s = Server::new(vec![
        (200, organization()),
        (200, entity_not_found()),
        (200, organization()),
    ]);
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_lnr"))
            .args(args)
            .env_remove("LINEAR_API_KEY")
            .env("XDG_CONFIG_HOME", d.path())
            .env("LNR_CREDENTIAL_STORE", "file")
            .env("LNR_API_URL", &s.url)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap()
    };
    let o = run(&["auth", "import-linear", "--workspace", "carraes"]);
    assert!(o.status.success(), "{}", output_text(&o));
    let o = run(&["issue", "view", "MON-1"]);
    assert_eq!(o.status.code(), Some(4), "{}", output_text(&o));
    let source = value(&o)["error"]["details"]["credential_source"]
        .as_str()
        .unwrap()
        .to_owned();
    let config = d.path().join("lnr/config.toml");
    assert_eq!(
        source,
        format!("stored workspace carraes in {}", config.display())
    );
    assert!(!output_text(&o).contains("fixture-private-key"));
}

#[test]
fn field_prints_key_value_lines_with_dotted_and_missing_paths() {
    let s = Server::new(vec![(200, issue())]);
    let o = s.run(&[
        "issue",
        "view",
        "ENG-7",
        "--field",
        "title,state.name,assignee.name,labels.name,parent.identifier",
    ]);
    assert!(o.status.success(), "{}", output_text(&o));
    assert_eq!(
        String::from_utf8_lossy(&o.stdout),
        "title: Fix login\nstate.name: In Progress\nassignee.name: \nlabels.name: bug, backend\nparent.identifier: \n"
    );
}

#[test]
fn single_field_prints_the_bare_value() {
    let s = Server::new(vec![(200, issue())]);
    let o = s.run(&["issue", "view", "ENG-7", "--field", "state.name"]);
    assert!(o.status.success(), "{}", output_text(&o));
    assert_eq!(String::from_utf8_lossy(&o.stdout), "In Progress\n");
    let o = Server::new(vec![(200, issue())]).run(&[
        "issue",
        "view",
        "ENG-7",
        "--field",
        "assignee.name",
    ]);
    assert!(o.status.success());
    assert_eq!(String::from_utf8_lossy(&o.stdout), "\n");
}

#[test]
fn unknown_top_level_field_fails_before_any_request() {
    let s = Server::new(vec![]);
    let o = s.run(&["issue", "view", "ENG-7", "--field", "title,status.name"]);
    assert_eq!(o.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(stderr.contains("Unknown field status"), "{stderr}");
    assert!(stderr.contains("Valid fields:"), "{stderr}");
    assert!(stderr.contains("state") && stderr.contains("assignee"));
    assert_eq!(s.count(), 0);
}

#[test]
fn field_with_explicit_json_returns_selected_values_in_the_envelope() {
    let s = Server::new(vec![(200, issue())]);
    let o = s.run(&["issue", "view", "ENG-7", "--json", "--field", "state.name"]);
    assert!(o.status.success());
    assert_eq!(value(&o)["data"], json!({"state.name":"In Progress"}));
}

#[test]
fn text_output_is_human_readable() {
    let s = Server::new(vec![(200, issue())]);
    let o = s.run(&["issue", "view", "ENG-7", "--output", "text"]);
    assert!(o.status.success(), "{}", output_text(&o));
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(text.starts_with("ENG-7  Fix login\n"), "{text}");
    assert!(text.contains("State:    In Progress\n"), "{text}");
    assert!(text.contains("Assignee: -\n"), "{text}");
    assert!(text.contains("Labels:   bug, backend\n"), "{text}");
    assert!(text.contains("Steps to reproduce\nline two"), "{text}");
    assert!(text.contains("Comments (1)") && text.contains("looks good"));
    assert!(!text.trim_start().starts_with('{'));
}

#[test]
fn json_view_exposes_labels_as_a_list() {
    let s = Server::new(vec![(200, issue())]);
    let o = s.run(&["issue", "view", "ENG-7"]);
    assert!(o.status.success());
    assert_eq!(value(&o)["data"]["labels"][1]["name"], "backend");
}

#[test]
fn text_and_field_output_strip_terminal_escapes() {
    let hostile = json!({"data":{"issue":{"id":"i","identifier":"ENG-7",
        "title":"Fix\u{1b}]8;;http://evil\u{7} login\u{1b}[2J","description":"ok\nline"}}});
    let o = Server::new(vec![(200, hostile.clone())]).run(&[
        "issue",
        "view",
        "ENG-7",
        "--no-comments",
        "--output",
        "text",
    ]);
    assert!(o.status.success(), "{}", output_text(&o));
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(
        !text.contains('\u{1b}') && !text.contains('\u{7}'),
        "{text:?}"
    );
    assert!(text.contains("ok\nline"));
    let o = Server::new(vec![(200, hostile)]).run(&["issue", "view", "ENG-7", "--field", "title"]);
    assert!(!String::from_utf8_lossy(&o.stdout).contains('\u{1b}'));
}
