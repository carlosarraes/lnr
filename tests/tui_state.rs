use crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers};
use lnr::{
    service::{page::Reply, project_overview::OverviewProject},
    tui::state::*,
};
use serde_json::json;
fn key(app: &mut App, k: K) -> Vec<Request> {
    app.handle_key(KeyEvent::new(k, KeyModifiers::NONE))
}
fn projects(ids: &[&str]) -> Payload {
    Payload::Projects(Reply {
        data: ids
            .iter()
            .map(|id| OverviewProject {
                project: serde_json::from_value(json!({"id":id,"name":id})).unwrap(),
                teams: vec![],
                involvement: vec![],
            })
            .collect(),
        meta: json!({"page":{"has_more":false,"complete":true}}),
    })
}
fn loaded() -> App {
    let mut a = App::new();
    let r = a.start().remove(1);
    a.apply(Response {
        generation: r.generation,
        kind: r.kind,
        result: Ok(projects(&["a", "b", "c"])),
    });
    a
}
#[test]
fn navigation_clamps_and_back_preserves_selection() {
    let mut a = loaded();
    key(&mut a, K::Down);
    assert_eq!(a.projects.selected, 1);
    let req = key(&mut a, K::Enter);
    assert!(matches!(req[0].kind,RequestKind::Issues{ref id,..} if id=="b"));
    assert_eq!(a.screen, Screen::Issues);
    key(&mut a, K::Esc);
    assert_eq!(a.screen, Screen::Projects);
    assert_eq!(a.projects.selected, 1);
    for _ in 0..9 {
        key(&mut a, K::Down);
    }
    assert_eq!(a.projects.selected, 2);
}
#[test]
fn search_and_filter_cancel_do_not_change_applied_scope() {
    let mut a = loaded();
    key(&mut a, K::Char('/'));
    key(&mut a, K::Char('x'));
    key(&mut a, K::Esc);
    assert!(a.filter.query.is_empty());
    key(&mut a, K::Char('/'));
    key(&mut a, K::Char('x'));
    let r = key(&mut a, K::Enter);
    assert_eq!(a.filter.query, "x");
    assert!(a.projects.items.is_empty());
    assert_eq!(r.len(), 1);
    key(&mut a, K::Char('f'));
    key(&mut a, K::Tab);
    key(&mut a, K::Tab);
    key(&mut a, K::Down);
    key(&mut a, K::Char(' '));
    key(&mut a, K::Esc);
    assert!(a.filter.involvement.is_empty());
    key(&mut a, K::Char('f'));
    key(&mut a, K::Tab);
    key(&mut a, K::Tab);
    key(&mut a, K::Down);
    key(&mut a, K::Char(' '));
    key(&mut a, K::Enter);
    assert_eq!(a.filter.involvement, vec!["lead"]);
}
#[test]
fn stale_responses_and_refresh_failure_preserve_current_data() {
    let mut a = loaded();
    let old = key(&mut a, K::Char('r')).pop().unwrap();
    let new = key(&mut a, K::Char('m')).remove(0);
    a.apply(Response {
        generation: old.generation,
        kind: old.kind,
        result: Ok(projects(&["old"])),
    });
    assert!(a.projects.items.is_empty());
    a.apply(Response {
        generation: new.generation,
        kind: new.kind,
        result: Ok(projects(&["new"])),
    });
    let stamp = a.refreshed;
    let r = key(&mut a, K::Char('r')).pop().unwrap();
    a.apply(Response {
        generation: r.generation,
        kind: r.kind,
        result: Err(lnr::error::AppError::new("network", "offline")),
    });
    assert_eq!(a.projects.items[0].project.id, "new");
    assert_eq!(a.refreshed, stamp);
    assert!(a.error.as_ref().unwrap().contains("offline"));
}
#[test]
fn append_deduplicates_and_repeated_cursor_stops_paging() {
    let mut a = loaded();
    a.projects.cursor = Some("next".into());
    a.projects.has_more = true;
    key(&mut a, K::Down);
    let r = key(&mut a, K::Char('n')).remove(0);
    let Payload::Projects(mut page) = projects(&["c", "d"]) else {
        panic!()
    };
    page.meta = json!({"page":{"has_more":true,"end_cursor":"next"}});
    a.apply(Response {
        generation: r.generation,
        kind: r.kind,
        result: Ok(Payload::Projects(page)),
    });
    assert_eq!(a.projects.items.len(), 4);
    assert_eq!(a.projects.selected, 1);
    assert!(!a.projects.has_more);
    assert!(!a.projects.complete);
    assert!(a.error.is_some());
}
#[test]
fn control_c_always_quits_and_unicode_search_backspace_is_safe() {
    let mut a = loaded();
    key(&mut a, K::Char('/'));
    key(&mut a, K::Char('界'));
    key(&mut a, K::Backspace);
    key(&mut a, K::Enter);
    assert!(a.filter.query.is_empty());
    key(&mut a, K::Char('f'));
    a.handle_key(KeyEvent::new(K::Char('c'), KeyModifiers::CONTROL));
    assert!(a.quit);
}

#[test]
fn failed_refresh_with_empty_partial_data_keeps_previous_rows() {
    let mut a = loaded();
    let request = key(&mut a, K::Char('r')).pop().unwrap();
    let mut error = lnr::error::AppError::new("network", "offline");
    error.data = Some(json!([]));
    a.apply(Response {
        generation: request.generation,
        kind: request.kind,
        result: Err(error),
    });
    assert_eq!(a.projects.items.len(), 3);
}
#[test]
fn issue_detail_and_comments_back_navigation_and_empty_enter() {
    let mut a = loaded();
    let request = key(&mut a, K::Enter).remove(0);
    a.apply(Response {
        generation: request.generation,
        kind: request.kind,
        result: Ok(Payload::Issues(Reply {
            data: vec![serde_json::from_value(json!({"id":"i","title":"Issue"})).unwrap()],
            meta: json!({"page":{"complete":true,"has_more":false}}),
        })),
    });
    let request = key(&mut a, K::Enter).remove(0);
    assert_eq!(a.screen, Screen::Detail);
    a.apply(Response {
        generation: request.generation,
        kind: request.kind,
        result: Ok(Payload::Issue(
            serde_json::from_value(json!({"id":"i","description":"body"})).unwrap(),
        )),
    });
    assert!(matches!(a.load_comments().unwrap().kind,RequestKind::Comments{ref id,..} if id=="i"));
    key(&mut a, K::Esc);
    assert_eq!(a.screen, Screen::Issues);
    assert_eq!(a.issues.items[0].id, "i");
    key(&mut a, K::Esc);
    assert_eq!(a.screen, Screen::Projects);
    let mut empty = App::new();
    assert!(key(&mut empty, K::Enter).is_empty());
}

#[test]
fn issue_refresh_failure_preserves_discussion() {
    let mut a = loaded();
    a.screen = Screen::Detail;
    a.issues.items = vec![serde_json::from_value(json!({"id":"i"})).unwrap()];
    a.issue = Some(a.issues.items[0].clone());
    a.comments.items = vec![serde_json::from_value(json!({"id":"c","body":"Keep this"})).unwrap()];
    let request = key(&mut a, K::Char('r')).pop().unwrap();
    assert_eq!(a.comments.items.len(), 1);
    a.apply(Response {
        generation: request.generation,
        kind: request.kind,
        result: Err(lnr::error::AppError::new("network", "offline")),
    });
    assert_eq!(a.comments.items[0].body, "Keep this");
}
