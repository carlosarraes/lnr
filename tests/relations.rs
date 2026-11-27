mod support;
use serde_json::json;
use support::*;
#[test]
fn blocks_preserves_direction() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"issueRelationCreate":{"success":true,"issueRelation":{"id":"r"}}}}),
    )]);
    let o = s.run(&[
        "issue",
        "relation",
        "add",
        "00000000-0000-0000-0000-000000000001",
        "--blocks",
        "00000000-0000-0000-0000-000000000002",
    ]);
    assert!(o.status.success(), "{:?}", value(&o));
    let r = s.requests.lock().unwrap();
    assert_eq!(r[0]["variables"]["input"]["type"], "blocks");
    assert_eq!(
        r[0]["variables"]["input"]["issueId"],
        "00000000-0000-0000-0000-000000000001"
    );
}
#[test]
fn relation_target_required() {
    let s = Server::new(vec![]);
    let o = s.run(&["issue", "relation", "add", "ENG-1"]);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(s.count(), 0);
}

fn issue_ref(id: &str, identifier: &str, title: &str) -> serde_json::Value {
    json!({"id":id,"identifier":identifier,"title":title})
}
fn viewed_issue() -> serde_json::Value {
    let me = issue_ref("i", "ENG-1", "Viewed");
    json!({"data":{"issue":{
        "id":"i","identifier":"ENG-1","title":"Viewed",
        "relations":page(json!([
            {"id":"r1","type":"blocks","issue":me,"relatedIssue":issue_ref("b","ENG-2","Downstream")},
            {"id":"r2","type":"related","issue":me,"relatedIssue":issue_ref("c","ENG-3","Sibling")},
            {"id":"r3","type":"duplicate","issue":me,"relatedIssue":issue_ref("d","ENG-7","Original")}
        ]),false,None),
        "inverseRelations":page(json!([
            {"id":"r4","type":"blocks","issue":issue_ref("e","ENG-4","Blocker"),"relatedIssue":me},
            {"id":"r5","type":"related","issue":issue_ref("f","ENG-5","Cousin"),"relatedIssue":me}
        ]),true,Some("inv2"))
    }}})
}

#[test]
fn view_groups_relations_in_both_directions() {
    let s = Server::new(vec![(200, viewed_issue())]);
    let o = s.run(&["issue", "view", "ENG-1", "--no-comments"]);
    assert!(o.status.success(), "{:?}", value(&o));
    let v = value(&o);
    let relations = &v["data"]["relations"];
    assert_eq!(
        relations["blocks"][0]["related_issue"]["identifier"],
        "ENG-2"
    );
    assert_eq!(relations["blocked_by"][0]["issue"]["identifier"], "ENG-4");
    assert_eq!(relations["related"].as_array().unwrap().len(), 2);
    assert_eq!(
        relations["duplicate_of"][0]["related_issue"]["identifier"],
        "ENG-7"
    );
    assert!(v["data"].get("inverse_relations").is_none());
    assert!(v["data"].get("comments").is_none());
    assert_eq!(v["meta"]["complete"], false);
    assert_eq!(v["meta"]["collections"]["relations"]["complete"], true);
    assert_eq!(
        v["meta"]["collections"]["inverse_relations"]["end_cursor"],
        "inv2"
    );
    let r = s.requests.lock().unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0]["variables"]["includeRelations"], true);
    assert_eq!(r[0]["variables"]["includeComments"], false);
}

#[test]
fn view_text_and_fields_show_blocks_blocked_by_and_related() {
    let o = Server::new(vec![(200, viewed_issue())]).run(&[
        "issue",
        "view",
        "ENG-1",
        "--no-comments",
        "--output",
        "text",
    ]);
    assert!(o.status.success());
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(text.contains("Relations (5)"), "{text}");
    assert!(
        text.contains("  blocks        ENG-2  Downstream\n"),
        "{text}"
    );
    assert!(text.contains("  blocked by    ENG-4  Blocker\n"), "{text}");
    assert!(text.contains("  related       ENG-3  Sibling\n"), "{text}");
    assert!(text.contains("  related       ENG-5  Cousin"), "{text}");
    assert!(text.contains("  duplicate of  ENG-7  Original"), "{text}");
    let o = Server::new(vec![(200, viewed_issue())]).run(&[
        "issue",
        "view",
        "ENG-1",
        "--no-comments",
        "--field",
        "relations.blocked_by.issue.identifier,relations.blocks.related_issue.identifier",
    ]);
    assert!(o.status.success());
    assert_eq!(
        String::from_utf8_lossy(&o.stdout),
        "relations.blocked_by.issue.identifier: ENG-4\nrelations.blocks.related_issue.identifier: ENG-2\n"
    );
    let o = Server::new(vec![(200, viewed_issue())]).run(&[
        "issue",
        "view",
        "ENG-1",
        "--no-comments",
        "--json",
        "--field",
        "relations",
    ]);
    assert!(o.status.success(), "{:?}", value(&o));
    assert_eq!(value(&o)["data"]["relations"]["blocked_by"][0]["id"], "r4");
}
