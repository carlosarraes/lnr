mod support;
use serde_json::json;
use support::*;
#[test]
fn add_comment_reads_stdin() {
    let s = Server::new(vec![
        (200, json!({"data":{"issue":{"id":"i"}}})),
        (
            200,
            json!({"data":{"commentCreate":{"success":true,"comment":{"id":"c","body":"Oi\n"}}}}),
        ),
    ]);
    let o = s.run_input(
        &["issue", "comment", "add", "ENG-1", "--body-file", "-"],
        Some("Oi\n"),
        true,
    );
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(
        s.requests.lock().unwrap()[1]["variables"]["input"]["body"],
        "Oi\n"
    );
}
#[test]
fn comment_list_pages() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issue":{"comments":page(json!([{"id":"c","body":"text"}]),false,None)}}}),
    )]);
    let o = s.run(&["issue", "comment", "list", "ENG-1"]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(value(&o)["data"][0]["id"], "c");
}
