mod support;
use serde_json::json;
use support::*;
#[test]
fn selected_mutation_is_never_retried() {
    let s = Server::new(vec![(503, json!({}))]);
    let o = s.run_input(
        &["api", "--query-file", "-", "--operation-name", "Write"],
        Some("query Read { viewer { id } } mutation Write { issueDelete(id: \"x\") { success } }"),
        true,
    );
    assert_eq!(value(&o)["error"]["code"], "uncertain_mutation");
    assert_eq!(s.count(), 1);
}
#[test]
fn ambiguous_operation_and_malformed_document_fail_locally() {
    for text in [
        "query A { viewer { id } } query B { viewer { id } }",
        "nonsense",
        "subscription { viewer { id } }",
    ] {
        let s = Server::new(vec![]);
        let o = s.run_input(&["api", "--query-file", "-"], Some(text), true);
        assert_eq!(o.status.code(), Some(2));
        assert_eq!(s.count(), 0);
    }
}
#[test]
fn no_double_stdin() {
    let s = Server::new(vec![]);
    let o = s.run_input(
        &["api", "--query-file", "-", "--variables-file", "-"],
        None,
        true,
    );
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(s.count(), 0);
}
#[test]
fn deprecated_field_error_names_replacement_and_keeps_details() {
    let s = Server::new(vec![(
        200,
        json!({"errors":[{"message":"deprecated","path":["issueSearch"],"extensions":{"code":"INPUT_ERROR","userPresentableMessage":"This endpoint deprecated."}}],"data":null}),
    )]);
    let o = s.run_input(
        &["api", "--query-file", "-"],
        Some("query { issueSearch(query: \"x\", first: 1) { nodes { id } } }"),
        true,
    );
    assert_eq!(o.status.code(), Some(8));
    let error = &value(&o)["error"];
    let message = error["message"].as_str().unwrap();
    assert!(
        message.contains("issueSearch") && message.contains("searchIssues"),
        "{message}"
    );
    assert_eq!(
        error["details"]["graphql_errors"][0]["path"][0],
        "issueSearch"
    );
    assert_eq!(
        error["details"]["graphql_errors"][0]["extensions"]["code"],
        "INPUT_ERROR"
    );
}
#[test]
fn api_help_points_to_dedicated_commands() {
    let s = Server::new(vec![]);
    let o = s.run(&["api", "--help"]);
    let help = String::from_utf8_lossy(&o.stdout);
    for command in ["issue relation add", "issue context", "issue list --search"] {
        assert!(help.contains(command), "{command} missing from:\n{help}");
    }
}

#[test]
fn positional_query_and_matching_flag_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("query.graphql");
    std::fs::write(&path, "query { viewer { id } }").unwrap();
    let path = path.to_str().unwrap();
    for args in [vec!["api", path], vec!["api", path, "--query-file", path]] {
        let s = Server::new(vec![(200, json!({"data":{"viewer":{"id":"u"}}}))]);
        let o = s.run(&args);
        assert!(o.status.success(), "{:?}", value(&o));
        assert_eq!(value(&o)["data"]["viewer"]["id"], "u");
    }
}

#[test]
fn missing_or_conflicting_query_files_fail_locally() {
    for args in [vec!["api"], vec!["api", "a", "--query-file", "b"]] {
        let s = Server::new(vec![]);
        let o = s.run(&args);
        assert_eq!(o.status.code(), Some(2));
        assert!(
            value(&o)["error"]["message"]
                .as_str()
                .unwrap()
                .contains("file path or --query-file <path>")
        );
        assert_eq!(s.count(), 0);
    }
}
