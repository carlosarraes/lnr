use lnr::{
    service::{page::Reply, project_overview::OverviewProject},
    tui::{render::render, state::*},
};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;
fn app() -> App {
    let mut a = App::new();
    let r = a.start().remove(1);
    a.apply(Response{generation:r.generation,kind:r.kind,result:Ok(Payload::Projects(Reply{data:vec![OverviewProject{project:serde_json::from_value(json!({"id":"p","name":"界面 Cafe\u{301}","progress":0.5,"description":"hello\u{1b}]52;c;secret\u{7}\nsecond line"})).unwrap(),teams:vec![],involvement:vec![]}],meta:json!({"page":{"has_more":false,"complete":true}})}))});
    a
}
fn draw(a: &mut App, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| render(f, a)).unwrap();
    t.backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}
#[test]
fn overview_is_readable_at_supported_sizes() {
    for (w, h) in [(120, 36), (80, 24), (60, 18)] {
        let s = draw(&mut app(), w, h);
        assert!(s.contains("50%"), "{s}");
        assert!(s.contains("Cafe"));
        assert!(s.contains("search"));
        assert!(!s.contains("Filter projects"));
        assert!(!s.contains('\u{1b}'));
        assert!(!s.contains('\u{7}'));
    }
}
#[test]
fn tiny_terminal_and_long_content_do_not_panic() {
    for (w, h) in [(40, 10), (1, 1), (0, 0)] {
        let s = draw(&mut app(), w, h);
        if w == 40 {
            assert!(s.contains("resize"));
        }
    }
    let mut a = app();
    a.scroll = u16::MAX;
    draw(&mut a, 60, 18);
    assert!(a.scroll < 100);
}
#[test]
fn missing_progress_is_not_zero_and_filter_is_an_overlay() {
    let mut a = app();
    a.projects.items[0].project.progress = None;
    let s = draw(&mut a, 120, 36);
    assert!(!s.contains("0%"));
    a.overlay = Some(Overlay::Filters {
        draft: Default::default(),
        group: 2,
        cursor: 1,
    });
    let s = draw(&mut a, 120, 36);
    assert!(s.contains("Filter projects"));
    assert!(s.contains("Lead"));
    assert!(s.contains("space"));
}
#[test]
fn selected_row_scrolls_into_view() {
    let mut a = app();
    for i in 0..50 {
        let mut p = a.projects.items[0].clone();
        p.project.id = i.to_string();
        p.project.name = Some(format!("Project {i}"));
        a.projects.items.push(p);
    }
    a.projects.selected = 50;
    let s = draw(&mut a, 80, 24);
    assert!(s.contains("Project 49"));
    assert!(a.projects.offset > 0);
}

#[test]
fn detail_scroll_can_reach_last_word_wrapped_line() {
    let mut a = app();
    a.projects.items[0].project.description = Some(
        (0..80)
            .map(|_| "abcdefghij klmnopqrst uvwxyz0123 456789abcd ")
            .collect::<String>()
            + "FINAL SENTENCE",
    );
    a.scroll = u16::MAX;
    let s = draw(&mut a, 60, 18);
    assert!(s.contains("FINAL SENTENCE"), "{s}");
}

#[test]
fn help_is_visible_below_minimum_size() {
    let mut a = app();
    a.overlay = Some(Overlay::Help);
    let s = draw(&mut a, 40, 10);
    assert!(s.contains("Keyboard"));
    assert!(s.contains("quit"));
}

#[test]
fn completed_toggle_changes_rows_counts_and_keeps_selected_detail_consistent() {
    let mut a = app();
    a.projects.items[0].project.progress = Some(1.0);
    a.projects.items[0].project.name = Some("Finished project".into());
    let mut active = a.projects.items[0].clone();
    active.project.id = "active".into();
    active.project.name = Some("Active project".into());
    active.project.progress = Some(0.999);
    a.projects.items.push(active);
    a.projects.selected = 1;
    let s = draw(&mut a, 120, 36);
    assert!(!s.contains("Finished project"));
    assert!(s.contains("1 shown"));
    assert!(s.contains("99%"));
    assert!(s.contains("tab"));
    assert!(s.contains("C completed"));
    a.show_completed = true;
    let s = draw(&mut a, 120, 36);
    assert!(s.contains("Finished project"));
    assert!(s.contains("100%"));
}
