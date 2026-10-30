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

#[test]
fn comments_alias_preserves_pagination() {
    let reply = json!({"data":{"issue":{"comments":page(json!([{"id":"c","body":"text"}]),true,Some("next"))}}});
    let s = Server::new(vec![(200, reply.clone()), (200, reply)]);
    let list = s.run(&[
        "issue", "comment", "list", "ENG-1", "--limit", "2", "--after", "previous",
    ]);
    let alias = s.run(&[
        "issue", "comments", "ENG-1", "--limit", "2", "--after", "previous",
    ]);
    assert!(alias.status.success(), "{:?}", value(&alias));
    assert_eq!(value(&alias), value(&list));
    assert_eq!(value(&alias)["meta"]["page"]["end_cursor"], "next");
    let r = s.requests.lock().unwrap();
    assert_eq!(r[0], r[1]);
}

#[test]
fn edit_comment_sends_only_id_and_body_and_prints_updated_comment() {
    for file in [false, true] {
        let s = Server::new(vec![(
            200,
            json!({"data":{"commentUpdate":{"success":true,"comment":{"id":"c","body":"Edited\n","createdAt":"2026-10-30T12:00:00Z","user":{"id":"u","name":"Carlos"}}}}}),
        )]);
        let args = if file {
            vec!["issue", "comment", "edit", "c", "--body-file", "-"]
        } else {
            vec!["issue", "comment", "edit", "c", "--body", "Edited\n"]
        };
        let o = s.run_input(&args, file.then_some("Edited\n"), true);
        assert!(o.status.success(), "{:?}", value(&o));
        assert_eq!(value(&o)["schema_version"], 1);
        assert_eq!(value(&o)["data"]["id"], "c");
        assert_eq!(value(&o)["data"]["body"], "Edited\n");
        assert_eq!(value(&o)["data"]["created_at"], "2026-10-30T12:00:00Z");
        let r = s.requests.lock().unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["operationName"], "EditComment");
        assert!(r[0]["query"].as_str().unwrap().contains("commentUpdate"));
        assert_eq!(
            r[0]["variables"],
            json!({"id":"c","input":{"body":"Edited\n"}})
        );
    }
}

#[test]
fn delete_comment_checks_mutation_success() {
    for success in [true, false] {
        let s = Server::new(vec![(
            200,
            json!({"data":{"commentDelete":{"success":success}}}),
        )]);
        let o = s.run(&["issue", "comment", "delete", "c"]);
        assert_eq!(o.status.code(), Some(if success { 0 } else { 8 }));
        if success {
            assert_eq!(value(&o)["data"]["success"], true);
        } else {
            assert_eq!(value(&o)["error"]["code"], "mutation_rejected");
        }
        let r = s.requests.lock().unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["operationName"], "DeleteComment");
        assert!(r[0]["query"].as_str().unwrap().contains("commentDelete"));
        assert_eq!(r[0]["variables"], json!({"id":"c"}));
    }
}

#[test]
fn comment_body_sources_are_exclusive_and_body_must_not_be_empty() {
    let s = Server::new(vec![]);
    for command in ["add", "edit"] {
        for sources in [
            vec![],
            vec!["--body", "  "],
            vec!["--body", "text", "--body-file", "-"],
        ] {
            let mut args = vec!["issue", "comment", command, "c"];
            args.extend(sources);
            let o = s.run(&args);
            assert_eq!(o.status.code(), Some(2));
            assert_eq!(value(&o)["error"]["code"], "invalid_input");
        }
    }
    let o = s.run(&["issue", "comment", "delete", " "]);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(s.count(), 0);
}

#[test]
fn mentions_resolve_users_insert_profile_urls_and_report_unique_users() {
    for command in ["add", "edit"] {
        let user1 = "00000000-0000-0000-0000-000000000011";
        let user2 = "00000000-0000-0000-0000-000000000012";
        let body = "https://linear.app/acme/profiles/carlos https://linear.app/acme/profiles/ana\n\nPlease review @literal\n";
        let mut replies = vec![];
        if command == "add" {
            replies.push((200, json!({"data":{"issue":{"id":"i"}}})));
        }
        replies.extend([
            (200, json!({"data":{"users":page(json!([{"id":user1,"name":"Carlos Arraes","displayName":"carlos","email":"carlos@example.com"}]),false,None)}})),
            (200, json!({"data":{"user":{"id":user1,"name":"Carlos Arraes","url":"https://linear.app/acme/profiles/carlos"}}})),
            (200, json!({"data":{"users":page(json!([{"id":user2,"name":"Ana","displayName":"ana","email":"ana@example.com"}]),false,None)}})),
            (200, json!({"data":{"user":{"id":user2,"name":"Ana","url":"https://linear.app/acme/profiles/ana"}}})),
            (200, json!({"data":{if command == "add" {"commentCreate"} else {"commentUpdate"}:{"success":true,"comment":{"id":"c","body":body}}}})),
        ]);
        let s = Server::new(replies);
        let o = s.run(&[
            "issue",
            "comment",
            command,
            "ENG-1",
            "--body",
            "Please review @literal\n",
            "--mention",
            "carlos",
            "--mention",
            "ana@example.com",
            "--mention",
            "carlos",
        ]);
        assert!(o.status.success(), "{:?}", value(&o));
        assert_eq!(value(&o)["data"]["body"], body);
        assert_eq!(
            value(&o)["data"]["mentioned_users"],
            json!([
                {"id":user1,"name":"Carlos Arraes","url":"https://linear.app/acme/profiles/carlos"},
                {"id":user2,"name":"Ana","url":"https://linear.app/acme/profiles/ana"}
            ])
        );
        let r = s.requests.lock().unwrap();
        let offset = usize::from(command == "add");
        assert_eq!(
            r[offset]["variables"]["filter"],
            json!({"or":[{"name":{"eq":"carlos"}},{"displayName":{"eq":"carlos"}},{"email":{"eq":"carlos"}}]})
        );
        assert_eq!(r[offset + 1]["variables"], json!({"id":user1}));
        assert!(r[offset + 1]["query"].as_str().unwrap().contains("url"));
        assert_eq!(
            r[offset + 2]["variables"]["filter"]["or"][2]["email"]["eq"],
            "ana@example.com"
        );
        assert_eq!(r.last().unwrap()["variables"]["input"]["body"], body);
        assert!(
            r.last().unwrap()["variables"]["input"]
                .get("bodyData")
                .is_none()
        );
        assert_eq!(r.len(), offset + 5);
    }
}

#[test]
fn unresolved_mention_prevents_mutation() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"users":page(json!([]),false,None)}}),
    )]);
    let o = s.run(&[
        "issue",
        "comment",
        "edit",
        "c",
        "--body",
        "Review",
        "--mention",
        "nobody",
    ]);
    assert_eq!(o.status.code(), Some(4));
    assert_eq!(s.count(), 1);
}

#[test]
fn mention_help_does_not_claim_notification_delivery() {
    let s = Server::new(vec![]);
    for command in ["add", "edit"] {
        let o = s.run(&["issue", "comment", command, "--help"]);
        assert!(o.status.success());
        let help = String::from_utf8(o.stdout).unwrap();
        assert!(help.contains("--mention"));
        assert!(help.contains("notification delivery is unverified"));
    }
}
