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
