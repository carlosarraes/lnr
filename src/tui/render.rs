//! A quiet, terminal-native presentation. All user-provided text is sanitized here.
use super::state::{App, Overlay, Page, Screen};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState,
        Wrap,
    },
};
const BG: Color = Color::Rgb(22, 25, 31);
const FG: Color = Color::Rgb(200, 204, 212);
const MUTED: Color = Color::Rgb(125, 133, 148);
const BLUE: Color = Color::Rgb(147, 180, 220);
const SELECTED: Color = Color::Rgb(41, 52, 67);
fn clean(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}
fn single(s: &str) -> String {
    clean(s).replace(['\n', '\t'], " ")
}
fn name(s: Option<&str>) -> String {
    single(s.unwrap_or("—"))
}
fn progress(p: Option<f64>, width: usize) -> String {
    match p.filter(|p| p.is_finite()) {
        Some(p) => {
            let p = p.clamp(0., 1.);
            let n = (p * width as f64).round() as usize;
            format!(
                "{}{} {:>3.0}%",
                "━".repeat(n),
                "─".repeat(width - n),
                p * 100.
            )
        }
        None => "—".into(),
    }
}
fn text(f: &mut Frame, area: Rect, s: impl Into<String>, color: Color) {
    f.render_widget(
        Paragraph::new(s.into()).style(Style::default().fg(color)),
        area,
    );
}
fn table<T>(
    f: &mut Frame,
    area: Rect,
    page: &mut Page<T>,
    rows: Vec<Row<'static>>,
    widths: Vec<Constraint>,
    header: Row<'static>,
) {
    let mut state = TableState::default()
        .with_offset(page.offset)
        .with_selected(if page.items.is_empty() {
            None
        } else {
            Some(page.selected)
        });
    f.render_stateful_widget(
        Table::new(rows, widths)
            .header(header.style(Style::default().fg(MUTED)).bottom_margin(1))
            .column_spacing(2)
            .row_highlight_style(Style::default().bg(SELECTED).fg(Color::White))
            .highlight_symbol("› "),
        area,
        &mut state,
    );
    page.offset = state.offset();
}
fn page_status<T>(page: &Page<T>) -> String {
    format!(
        "{} loaded{}",
        page.items.len(),
        if page.has_more {
            " · more available · n next"
        } else if !page.complete {
            " · incomplete"
        } else {
            ""
        }
    )
}
pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    f.render_widget(Block::default().style(Style::default().bg(BG).fg(FG)), area);
    if area.width < 60 || area.height < 18 {
        text(
            f,
            area,
            "Please resize to at least 60 × 18.\nq quit · Ctrl-C exit · ? help",
            MUTED,
        );
        return;
    }
    let body = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let [header, gap, scope, status, main, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(body);
    let _ = gap;
    let (workspace, viewer) = app
        .catalog
        .as_ref()
        .map(|c| {
            (
                c.viewer
                    .organization
                    .as_ref()
                    .map(|o| o.url_key.as_str())
                    .unwrap_or("workspace"),
                c.viewer.viewer.name.as_deref().unwrap_or(""),
            )
        })
        .unwrap_or(("workspace", ""));
    let [brand, user] =
        Layout::horizontal([Constraint::Min(1), Constraint::Length(22)]).areas(header);
    text(f, brand, format!("lnr   /   {}", single(workspace)), BLUE);
    f.render_widget(
        Paragraph::new(single(viewer))
            .alignment(ratatui::layout::Alignment::Right)
            .style(Style::default().fg(BLUE)),
        user,
    );
    let title = match app.screen {
        Screen::Projects => format!(
            "Projects   {}",
            if app.filter.mine {
                "All   [Mine]"
            } else {
                "[All]   Mine"
            }
        ),
        Screen::Issues => format!(
            "{} / Issues",
            name(
                app.selected_project()
                    .and_then(|p| p.project.name.as_deref())
            )
        ),
        Screen::Detail => format!(
            "{} / Discussion",
            name(app.selected_issue().and_then(|i| i.identifier.as_deref()))
        ),
    };
    text(f, scope, title, FG);
    let note = if let Some(Overlay::Search(draft)) = &app.overlay {
        format!("/ {}▏   enter apply · esc cancel", single(draft))
    } else if app.loading {
        "Loading…".into()
    } else if let Some(e) = app.error.as_ref().or(app.catalog_error.as_ref()) {
        format!(
            "{} · r retry{}",
            single(e),
            app.refreshed
                .map(|t| format!(" · last success {}s ago", t.elapsed().as_secs()))
                .unwrap_or_default()
        )
    } else {
        let page = match app.screen {
            Screen::Projects => page_status(&app.projects),
            Screen::Issues => page_status(&app.issues),
            Screen::Detail => format!("Comments: {}", page_status(&app.comments)),
        };
        let mut summary = vec![page];
        if app.screen == Screen::Projects {
            if !app.filter.query.is_empty() {
                summary.push(format!("search: {}", single(&app.filter.query)));
            }
            for (group, ids) in [
                (0, &app.filter.team_ids),
                (1, &app.filter.status_ids),
                (2, &app.filter.involvement),
            ] {
                let choices = app.filter_choices(group);
                summary.extend(ids.iter().map(|id| {
                    choices
                        .iter()
                        .find(|(v, _)| v == id)
                        .map(|(_, n)| single(n))
                        .unwrap_or_else(|| single(id))
                }));
            }
        }
        summary.join(" · ")
    };
    text(f, status, note, MUTED);
    match app.screen {
        Screen::Projects => {
            let [list, details] = Layout::vertical([
                Constraint::Min(3),
                Constraint::Length(if main.height > 17 { 8 } else { 5 }),
            ])
            .areas(main);
            let wide = list.width >= 100;
            let status_visible = list.width >= 74;
            let bar = if wide { 14 } else { 8 };
            let spaced = list.height >= app.projects.items.len() as u16 * 2 + 2;
            let mut widths = vec![Constraint::Min(12)];
            let mut headers = vec![Cell::from("PROJECT")];
            if status_visible {
                widths.push(Constraint::Length(16));
                headers.push(Cell::from("STATUS"));
            }
            widths.push(Constraint::Length(bar as u16 + 6));
            headers.push(Cell::from("PROGRESS"));
            if wide {
                widths.push(Constraint::Length(12));
                headers.push(Cell::from("TARGET"));
            }
            let rows = app
                .projects
                .items
                .iter()
                .map(|p| {
                    let mut cells = vec![Cell::from(name(p.project.name.as_deref()))];
                    if status_visible {
                        cells.push(Cell::from(name(
                            p.project.status.as_ref().and_then(|s| s.name.as_deref()),
                        )));
                    }
                    cells.push(
                        Cell::from(progress(p.project.progress, bar))
                            .style(Style::default().fg(BLUE)),
                    );
                    if wide {
                        cells.push(Cell::from(name(p.project.target_date.as_deref())));
                    }
                    Row::new(cells).bottom_margin(u16::from(spaced))
                })
                .collect();
            table(f, list, &mut app.projects, rows, widths, Row::new(headers));
            if app.projects.items.is_empty() && !app.loading {
                text(
                    f,
                    list.inner(Margin {
                        horizontal: 0,
                        vertical: 2,
                    }),
                    "No projects match. / search · f filter",
                    MUTED,
                );
            }
            let mut lines = vec![];
            if let Some(p) = app.selected_project() {
                lines.push(Line::styled(
                    format!(
                        "{}   /   {}   /   {}",
                        name(p.project.name.as_deref()),
                        p.teams
                            .iter()
                            .map(|t| name(t.name.as_deref()))
                            .collect::<Vec<_>>()
                            .join(", "),
                        name(p.project.lead.as_ref().and_then(|l| l.name.as_deref()))
                    ),
                    Style::default().fg(BLUE),
                ));
                lines.push(Line::styled(
                    if p.involvement.is_empty() {
                        "No involvement".into()
                    } else {
                        format!("You: {}", p.involvement.join(", "))
                    },
                    Style::default().fg(MUTED),
                ));
                lines.push(Line::from(""));
                lines.extend(
                    clean(p.project.description.as_deref().unwrap_or("No description"))
                        .lines()
                        .map(|s| Line::from(s.to_string())),
                );
            }
            detail(f, details, lines, app);
        }
        Screen::Issues => {
            let rows = app
                .issues
                .items
                .iter()
                .map(|i| {
                    Row::new(vec![
                        name(i.identifier.as_deref()),
                        name(i.title.as_deref()),
                        name(i.state.as_ref().and_then(|s| s.name.as_deref())),
                        name(i.assignee.as_ref().and_then(|s| s.name.as_deref())),
                        match i.priority.map(|p| p as i32) {
                            Some(1) => "Urgent",
                            Some(2) => "High",
                            Some(3) => "Normal",
                            Some(4) => "Low",
                            _ => "—",
                        }
                        .into(),
                    ])
                })
                .collect();
            table(
                f,
                main,
                &mut app.issues,
                rows,
                vec![
                    Constraint::Length(12),
                    Constraint::Min(12),
                    Constraint::Length(12),
                    Constraint::Length(12),
                    Constraint::Length(6),
                ],
                Row::new(["ISSUE", "TITLE", "STATUS", "ASSIGNEE", "PRIO"]),
            );
            if app.issues.items.is_empty() && !app.loading {
                text(f, main, "No issues in this project.", MUTED);
            }
        }
        Screen::Detail => {
            let mut lines = vec![];
            if let Some(i) = &app.issue {
                lines.push(Line::styled(
                    name(i.title.as_deref()),
                    Style::default().fg(BLUE),
                ));
                lines.push(Line::from(""));
                lines.extend(
                    clean(i.description.as_deref().unwrap_or("No description"))
                        .lines()
                        .map(|s| Line::from(s.to_string())),
                );
                lines.push(Line::from(""));
                lines.push(Line::styled("Discussion", Style::default().fg(BLUE)));
                for c in &app.comments.items {
                    lines.push(Line::from(""));
                    lines.push(Line::styled(
                        format!(
                            "{}   {}",
                            name(c.user.as_ref().and_then(|u| u.name.as_deref())),
                            name(c.created_at.as_deref())
                        ),
                        Style::default().fg(MUTED),
                    ));
                    lines.extend(clean(&c.body).lines().map(|s| Line::from(s.to_string())));
                }
                if app.comments.items.is_empty() && !app.loading {
                    lines.push(Line::from("No comments loaded"));
                }
            }
            detail(f, main, lines, app);
        }
    }
    text(
        f,
        footer,
        if app.screen == Screen::Projects {
            "↑↓ move   enter open   / search   m mine   f filter   ? help"
        } else {
            "↑↓ move   enter open   esc back   ? help"
        },
        MUTED,
    );
    match &app.overlay {
        Some(Overlay::Help) => help(f, body),
        Some(Overlay::Filters {
            draft,
            group,
            cursor,
        }) => {
            let popup = center(body, 64, 16);
            f.render_widget(Clear, popup);
            let inner = popup.inner(Margin {
                horizontal: 2,
                vertical: 1,
            });
            f.render_widget(Block::default().style(Style::default().bg(SELECTED)), popup);
            let [title, tabs, list, keys] = Layout::vertical([
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Min(1),
                Constraint::Length(2),
            ])
            .areas(inner);
            text(f, title, "Filter projects", BLUE);
            text(
                f,
                tabs,
                ["team", "status", "role"]
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        if i == *group {
                            format!("[{s}]")
                        } else {
                            s.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("   "),
                FG,
            );
            let chosen = match group {
                0 => &draft.team_ids,
                1 => &draft.status_ids,
                _ => &draft.involvement,
            };
            let items: Vec<_> = app
                .filter_choices(*group)
                .into_iter()
                .map(|(id, n)| {
                    ListItem::new(format!(
                        "[{}] {}",
                        if if id.is_empty() {
                            chosen.is_empty()
                        } else {
                            chosen.contains(&id)
                        } {
                            "x"
                        } else {
                            " "
                        },
                        single(&n)
                    ))
                })
                .collect();
            let mut state = ListState::default().with_selected(Some(*cursor));
            f.render_stateful_widget(
                List::new(items)
                    .highlight_style(Style::default().bg(BLUE).fg(BG))
                    .highlight_symbol("› "),
                list,
                &mut state,
            );
            text(
                f,
                keys,
                "tab group · space toggle\nenter apply · esc cancel",
                MUTED,
            );
        }
        _ => {}
    }
}
fn detail(f: &mut Frame, area: Rect, lines: Vec<Line<'static>>, app: &mut App) {
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(MUTED))
            .padding(ratatui::widgets::Padding::new(0, 0, 1, 0)),
    );
    let total = paragraph.line_count(area.width);
    app.scroll = app.scroll.min(
        total
            .saturating_sub(area.height as usize)
            .min(u16::MAX as usize) as u16,
    );
    f.render_widget(paragraph.scroll((app.scroll, 0)), area);
}

fn center(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}
fn help(f: &mut Frame, area: Rect) {
    let popup = center(area, 64, 20);
    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled("Keyboard", Style::default().fg(BLUE))),
            Line::from(""),
            Line::from("↑↓ / j k    Move selection / scroll issue"),
            Line::from("enter       Open project or issue"),
            Line::from("esc         Back / cancel"),
            Line::from("m           All / my projects"),
            Line::from("/           Search project names"),
            Line::from("f           Filter team, status, involvement"),
            Line::from("tab / space Filter group / toggle choice"),
            Line::from("PgUp/PgDn   Scroll details"),
            Line::from("n           Load next page"),
            Line::from("r           Refresh / retry"),
            Line::from("q           Quit overview"),
            Line::from("Ctrl-C      Exit any screen"),
            Line::from(""),
            Line::from("esc close"),
        ])
        .style(Style::default().bg(SELECTED).fg(FG))
        .block(Block::default().padding(ratatui::widgets::Padding::uniform(1))),
        popup,
    );
}
