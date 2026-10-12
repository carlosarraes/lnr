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
