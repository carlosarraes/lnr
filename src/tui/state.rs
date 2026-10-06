//! Pure UI transitions. Requests leave this module; only matching responses enter it.
use crate::{
    error::Result,
    model::{Comment, Issue},
    service::{
        page::{PageRequest, Reply},
        project_overview::{OverviewCatalog, OverviewFilter, OverviewProject},
    },
};
use crossterm::event::{KeyCode as K, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum RequestKind {
    Catalog,
    Projects {
        filter: OverviewFilter,
        page: PageRequest,
    },
    Issues {
        id: String,
        page: PageRequest,
    },
    Issue {
        id: String,
    },
    Comments {
        id: String,
        page: PageRequest,
    },
}
#[derive(Debug, Clone)]
pub struct Request {
    pub generation: u64,
    pub kind: RequestKind,
}
pub struct Response {
    pub generation: u64,
    pub kind: RequestKind,
    pub result: Result<Payload>,
}
pub enum Payload {
    Catalog(OverviewCatalog),
    Projects(Reply<Vec<OverviewProject>>),
    Issues(Reply<Vec<Issue>>),
    Issue(Box<Issue>),
    Comments(Reply<Vec<Comment>>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Projects,
    Issues,
    Detail,
}
#[derive(Debug, Clone)]
pub enum Overlay {
    Help,
    Search(String),
    Filters {
        draft: OverviewFilter,
        group: usize,
        cursor: usize,
    },
}
#[derive(Debug)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub selected: usize,
    pub offset: usize,
    pub cursor: Option<String>,
    pub has_more: bool,
    pub complete: bool,
    seen: Vec<String>,
}
impl<T> Default for Page<T> {
    fn default() -> Self {
        Self {
            items: vec![],
            selected: 0,
            offset: 0,
            cursor: None,
            has_more: false,
            complete: false,
            seen: vec![],
        }
    }
}
impl<T> Page<T> {
    fn move_by(&mut self, delta: isize) {
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.items.len().saturating_sub(1));
    }
    fn ingest(
        &mut self,
        reply: Reply<Vec<T>>,
        append: bool,
        id: impl Fn(&T) -> &str,
    ) -> Option<String> {
        let old = self.items.get(self.selected).map(|x| id(x).to_string());
        if !append {
            self.items.clear();
            self.seen.clear();
            self.offset = 0;
        }
        for item in reply.data {
            if !self.items.iter().any(|x| id(x) == id(&item)) {
                self.items.push(item);
            }
        }
        self.selected = old
            .and_then(|old| self.items.iter().position(|x| id(x) == old))
            .unwrap_or(0);
        let previous_cursor = self.cursor.clone();
        let p = &reply.meta["page"];
        self.cursor = p["end_cursor"].as_str().map(str::to_owned);
        self.has_more = p["has_more"] == true;
        self.complete = p["has_more"] == false && p["complete"] == true;
        if self.has_more {
            let valid = self.cursor.as_ref().is_some_and(|c| {
                !c.is_empty()
                    && !self.seen.contains(c)
                    && (!append || previous_cursor.as_ref() != Some(c))
            });
            if !valid {
                self.has_more = false;
                self.complete = false;
                return Some("Pagination cursor missing or repeated; refresh to retry".into());
            }
            self.seen.push(self.cursor.clone().unwrap());
        } else if !self.complete {
            return Some("Partial data; refresh to retry".into());
        }
        None
    }
}
pub struct App {
    pub screen: Screen,
    pub overlay: Option<Overlay>,
    pub filter: OverviewFilter,
    pub catalog: Option<OverviewCatalog>,
    pub catalog_error: Option<String>,
    pub projects: Page<OverviewProject>,
    pub issues: Page<Issue>,
    pub comments: Page<Comment>,
    pub issue: Option<Issue>,
    pub loading: bool,
    pub error: Option<String>,
    pub refreshed: Option<Instant>,
    pub scroll: u16,
    pub quit: bool,
    generation: u64,
    catalog_generation: u64,
}
impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Projects,
            overlay: None,
            filter: Default::default(),
            catalog: None,
            catalog_error: None,
            projects: Default::default(),
            issues: Default::default(),
            comments: Default::default(),
            issue: None,
            loading: false,
            error: None,
            refreshed: None,
            scroll: 0,
            quit: false,
            generation: 0,
            catalog_generation: 0,
        }
    }
    pub fn start(&mut self) -> Vec<Request> {
        self.catalog_generation += 1;
        let catalog = Request {
            generation: self.catalog_generation,
            kind: RequestKind::Catalog,
        };
        vec![catalog, self.request_projects(false)]
    }
    fn request(&mut self, kind: RequestKind) -> Request {
        self.generation += 1;
        self.loading = true;
        self.error = None;
        Request {
            generation: self.generation,
            kind,
        }
    }
    fn request_projects(&mut self, next: bool) -> Request {
        self.request(RequestKind::Projects {
            filter: self.filter.clone(),
            page: PageRequest {
                after: if next {
                    self.projects.cursor.clone()
                } else {
                    None
                },
                ..Default::default()
            },
        })
    }
    fn changed_scope(&mut self) -> Request {
        self.projects = Page::default();
        self.scroll = 0;
        self.request_projects(false)
    }
    pub fn selected_project(&self) -> Option<&OverviewProject> {
        self.projects.items.get(self.projects.selected)
    }
    pub fn selected_issue(&self) -> Option<&Issue> {
        self.issues.items.get(self.issues.selected)
    }
    pub fn filter_choices(&self, group: usize) -> Vec<(String, String)> {
        let mut values = vec![(
            String::new(),
            if group == 2 { "Any involvement" } else { "All" }.into(),
        )];
        match group {
            0 | 1 => {
                if let Some(c) = &self.catalog {
                    values.extend(
                        (if group == 0 { &c.teams } else { &c.statuses })
                            .iter()
                            .map(|e| {
                                (e.id.clone(), e.name.clone().unwrap_or_else(|| e.id.clone()))
                            }),
                    );
                }
            }
            _ => values.extend(
                [
                    ("lead", "Lead"),
                    ("member", "Member"),
                    ("assignee", "Assigned issues"),
                ]
                .map(|(id, n)| (id.into(), n.into())),
            ),
        }
        values
    }
    pub fn accepts(&self, response: &Response) -> bool {
        response.generation
            == if matches!(response.kind, RequestKind::Catalog) {
                self.catalog_generation
            } else {
                self.generation
            }
    }
    pub fn apply(&mut self, response: Response) {
        if matches!(response.kind, RequestKind::Catalog) {
            if response.generation == self.catalog_generation {
                match response.result {
                    Ok(Payload::Catalog(c)) => {
                        self.catalog = Some(c);
                        self.catalog_error = None;
                    }
                    Err(e) => self.catalog_error = Some(e.message),
                    _ => {}
                }
            }
            return;
        }
        if response.generation != self.generation {
            return;
        }
        self.loading = false;
        let append = match &response.kind {
            RequestKind::Projects { page, .. }
            | RequestKind::Issues { page, .. }
            | RequestKind::Comments { page, .. } => page.after.is_some(),
            _ => false,
        };
        match response.result {
            Ok(payload) => {
                self.error = match payload {
                    Payload::Projects(r) => self.projects.ingest(r, append, |p| &p.project.id),
                    Payload::Issues(r) => self.issues.ingest(r, append, |i| &i.id),
                    Payload::Comments(r) => self.comments.ingest(r, append, |c| &c.id),
                    Payload::Issue(i) => {
                        self.issue = Some(*i);
                        None
                    }
                    Payload::Catalog(_) => None,
                };
                self.refreshed = Some(Instant::now());
            }
            Err(e) => {
                // Preserve typed partial pages from the service, without claiming completion.
                if let Some(data) = e
                    .data
                    .clone()
                    .filter(|v| v.as_array().is_some_and(|a| !a.is_empty()))
                {
                    let meta = serde_json::json!({"page":{"has_more":null,"complete":false}});
                    match response.kind {
                        RequestKind::Projects { .. } => {
                            if let Ok(data) = serde_json::from_value(data) {
                                self.projects
                                    .ingest(Reply { data, meta }, append, |p| &p.project.id);
                            }
                        }
                        RequestKind::Issues { .. } => {
                            if let Ok(data) = serde_json::from_value(data) {
                                self.issues.ingest(Reply { data, meta }, append, |i| &i.id);
                            }
                        }
                        RequestKind::Comments { .. } => {
                            if let Ok(data) = serde_json::from_value(data) {
                                self.comments
                                    .ingest(Reply { data, meta }, append, |c| &c.id);
                            }
                        }
                        _ => {}
                    }
                }
                self.error = Some(e.message);
            }
        }
    }
    pub fn handle_key(&mut self, event: KeyEvent) -> Vec<Request> {
        if event.kind == KeyEventKind::Release {
            return vec![];
        }
        if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == K::Char('c') {
            self.quit = true;
            return vec![];
        }
        if event
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return vec![];
        }
        let key = event.code;
        if let Some(overlay) = self.overlay.take() {
            match overlay {
                Overlay::Help => {
                    if !matches!(key, K::Esc | K::Char('?') | K::Enter) {
                        self.overlay = Some(Overlay::Help);
                    }
                }
                Overlay::Search(mut draft) => {
                    match key {
                        K::Enter => {
                            self.filter.query = draft;
                            return vec![self.changed_scope()];
                        }
                        K::Esc => return vec![],
                        K::Backspace => {
                            draft.pop();
                        }
                        K::Char(c) => draft.push(c),
                        _ => {}
                    }
                    self.overlay = Some(Overlay::Search(draft));
                }
                Overlay::Filters {
                    mut draft,
                    mut group,
                    mut cursor,
                } => {
                    match key {
                        K::Esc => return vec![],
                        K::Enter => {
                            self.filter = draft;
                            return vec![self.changed_scope()];
                        }
                        K::Tab => {
                            group = (group + 1) % 3;
                            cursor = 0;
                        }
                        K::BackTab => {
                            group = (group + 2) % 3;
                            cursor = 0;
                        }
                        K::Down | K::Char('j') => {
                            cursor = (cursor + 1) % self.filter_choices(group).len()
                        }
                        K::Up | K::Char('k') => cursor = cursor.saturating_sub(1),
                        K::Char(' ') => {
                            let choices = self.filter_choices(group);
                            let value = &choices[cursor.min(choices.len() - 1)].0;
                            let values = match group {
                                0 => &mut draft.team_ids,
                                1 => &mut draft.status_ids,
                                _ => &mut draft.involvement,
                            };
                            if value.is_empty() {
                                values.clear();
                            } else if values.contains(value) {
                                values.retain(|x| x != value);
                            } else {
                                values.push(value.clone());
                            }
                        }
                        _ => {}
                    }
                    self.overlay = Some(Overlay::Filters {
                        draft,
                        group,
                        cursor,
                    });
                }
            }
            return vec![];
        }
        match key {
            K::Char('?') => self.overlay = Some(Overlay::Help),
            K::Char('q') if self.screen == Screen::Projects => self.quit = true,
            K::Esc => {
                self.generation += 1;
                self.loading = false;
                self.error = None;
                self.scroll = 0;
                self.screen = match self.screen {
                    Screen::Detail => Screen::Issues,
                    _ => Screen::Projects,
                };
            }
            K::Char('m') if self.screen == Screen::Projects => {
                self.filter.mine = !self.filter.mine;
                return vec![self.changed_scope()];
            }
            K::Char('/') if self.screen == Screen::Projects => {
                self.overlay = Some(Overlay::Search(self.filter.query.clone()))
            }
            K::Char('f') if self.screen == Screen::Projects => {
                self.overlay = Some(Overlay::Filters {
                    draft: self.filter.clone(),
                    group: 0,
                    cursor: 0,
                })
            }
            K::Down | K::Char('j') | K::Up | K::Char('k') => {
                let delta = if matches!(key, K::Up | K::Char('k')) {
                    -1
                } else {
                    1
                };
                match self.screen {
                    Screen::Projects => self.projects.move_by(delta),
                    Screen::Issues => self.issues.move_by(delta),
                    Screen::Detail => self.scroll = self.scroll.saturating_add_signed(delta as i16),
                };
                if self.screen != Screen::Detail {
                    self.scroll = 0;
                }
            }
            K::PageDown => self.scroll = self.scroll.saturating_add(5),
            K::PageUp => self.scroll = self.scroll.saturating_sub(5),
            K::Enter => match self.screen {
                Screen::Projects => {
                    if let Some(p) = self.selected_project() {
                        let id = p.project.id.clone();
                        self.screen = Screen::Issues;
                        self.issues = Page::default();
                        self.scroll = 0;
                        return vec![self.request(RequestKind::Issues {
                            id,
                            page: PageRequest::default(),
                        })];
                    }
                }
                Screen::Issues => {
                    if let Some(i) = self.selected_issue() {
                        let id = i.id.clone();
                        self.screen = Screen::Detail;
                        self.issue = None;
                        self.comments = Page::default();
                        self.scroll = 0;
                        return vec![self.request(RequestKind::Issue { id })];
                    }
                }
                _ => {}
            },
            K::Char('r') => {
                let mut requests = vec![];
                if self.catalog.is_none() {
                    self.catalog_generation += 1;
                    requests.push(Request {
                        generation: self.catalog_generation,
                        kind: RequestKind::Catalog,
                    });
                }
                if let Some(r) = self.refresh(false) {
                    requests.push(r);
                }
                return requests;
            }
            K::Char('n') if !self.loading => {
                if let Some(r) = self.refresh(true) {
                    return vec![r];
                }
            }
            _ => {}
        }
        vec![]
    }
    pub fn load_comments(&mut self) -> Option<Request> {
        self.issue.as_ref().map(|i| i.id.clone()).map(|id| {
            self.request(RequestKind::Comments {
                id,
                page: PageRequest::default(),
            })
        })
    }
    fn refresh(&mut self, next: bool) -> Option<Request> {
        match self.screen {
            Screen::Projects => {
                if !next || self.projects.has_more {
                    Some(self.request_projects(next))
                } else {
                    None
                }
            }
            Screen::Issues => {
                if next && !self.issues.has_more {
                    return None;
                }
                let id = self.selected_project()?.project.id.clone();
                Some(self.request(RequestKind::Issues {
                    id,
                    page: PageRequest {
                        after: if next {
                            self.issues.cursor.clone()
                        } else {
                            None
                        },
                        ..Default::default()
                    },
                }))
            }
            Screen::Detail => {
                let id = self.selected_issue()?.id.clone();
                if next {
                    if !self.comments.has_more {
                        return None;
                    }
                    Some(self.request(RequestKind::Comments {
                        id,
                        page: PageRequest {
                            after: self.comments.cursor.clone(),
                            ..Default::default()
                        },
                    }))
                } else {
                    Some(self.request(RequestKind::Issue { id }))
                }
            }
        }
    }
}
