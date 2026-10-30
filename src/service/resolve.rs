use super::{
    Service,
    page::{PageRequest, Pages},
};
use crate::error::{AppError, Result};
use serde_json::json;
impl Service {
    pub async fn resolve(&self, kind: &str, reference: &str, team: Option<&str>) -> Result<String> {
        if reference.trim().is_empty() {
            return Err(AppError::input("Reference must not be empty"));
        }
        if is_uuid(reference) {
            return Ok(reference.into());
        }
        let key = format!("{kind}:{team:?}:{reference}");
        if let Some(id) = self.resolved.lock().await.get(&key) {
            return Ok(id.clone());
        }
        if kind == "issue" {
            let issue = self.view_issue(reference).await?;
            return Ok(issue.id);
        }
        if kind == "project" {
            let id = self.resolve_project(reference).await?;
            self.resolved.lock().await.insert(key, id.clone());
            return Ok(id);
        }
        let (operation, root) = catalog(kind)?;
        let mut filter = if kind == "team" {
            json!({"or":[{"key":{"eq":reference}},{"name":{"eq":reference}}]})
        } else if kind == "user" && reference == "me" {
            json!({"isMe":{"eq":true}})
        } else if kind == "user" {
            json!({"or":[{"name":{"eq":reference}},{"displayName":{"eq":reference}},{"email":{"eq":reference}}]})
        } else {
            json!({"name":{"eq":reference}})
        };
        if let Some(team) = team {
            if kind == "label" {
                filter["or"] = label_scope(team)["or"].clone();
            } else {
                filter["team"] = json!({"id":{"eq":team}});
            }
        }
        let reply = self
            .list(
                operation,
                root,
                filter,
                PageRequest {
                    all: true,
                    ..Default::default()
                },
            )
            .await?;
        let nodes = reply
            .data
            .as_array()
            .ok_or_else(|| AppError::new("api", "Invalid reference results"))?;
        if nodes.is_empty() {
            return Err(AppError::new(
                "not_found",
                format!("No {kind} matches {reference}"),
            ));
        }
        if nodes.len() > 1 {
            let mut e = AppError::new("ambiguous", format!("Multiple {kind} matches; use an ID"));
            e.details = (json!({"candidates":nodes})).into();
            return Err(e);
        }
        let id = nodes[0]["id"]
            .as_str()
            .ok_or_else(|| AppError::new("api", "Reference has no id"))?
            .to_owned();
        self.resolved.lock().await.insert(key, id.clone());
        Ok(id)
    }

    async fn resolve_project(&self, reference: &str) -> Result<String> {
        let mut pages = Pages::new(None);
        let mut workspace = None;
        loop {
            let data = self
                .query("ResolveProjects", json!({"first":250,"after":pages.cursor}))
                .await?;
            if workspace.is_none() {
                workspace = data["organization"]["urlKey"].as_str().map(str::to_owned);
            }
            pages.ingest(&data["projects"])?;
            if !pages.has_more() {
                break;
            }
        }
        let needle = reference.to_lowercase();
        let mut matches: Vec<_> = pages
            .items
            .iter()
            .filter(|p| {
                p["name"]
                    .as_str()
                    .is_some_and(|name| name.to_lowercase() == needle)
            })
            .collect();
        if matches.is_empty() {
            matches = pages
                .items
                .iter()
                .filter(|p| project_keys(p).iter().any(|key| key == &needle))
                .collect();
        }
        if matches.is_empty() {
            matches = pages
                .items
                .iter()
                .filter(|p| project_keys(p).iter().any(|key| key.starts_with(&needle)))
                .collect();
        }
        if matches.len() == 1 {
            return matches[0]["id"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| AppError::new("api", "Project reference has no id"));
        }
        let missing = matches.is_empty();
        let mut candidates = if missing {
            pages.items.iter().collect::<Vec<_>>()
        } else {
            matches
        };
        candidates.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        let candidates: Vec<_> = candidates
            .into_iter()
            .take(10)
            .map(|p| json!({"id":p["id"],"name":p["name"]}))
            .collect();
        let names = candidates
            .iter()
            .filter_map(|p| p["name"].as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let workspace = workspace
            .as_deref()
            .unwrap_or("unknown; run lnr auth status --check");
        let problem = if missing {
            "No project matches"
        } else {
            "Multiple projects match"
        };
        let names = if names.is_empty() {
            "none accessible"
        } else {
            &names
        };
        let mut error = AppError::new(
            if missing { "not_found" } else { "ambiguous" },
            format!(
                "{problem} {reference:?} in workspace {workspace}. Candidates (up to 10): {names}. Use an exact project name or UUID. To choose another workspace, use --workspace SLUG; unset LINEAR_API_KEY first if set. Run lnr auth status to list workspaces."
            ),
        );
        error.details = Box::new(json!({"workspace":workspace,"candidates":candidates}));
        Err(error)
    }
}

fn project_keys(project: &serde_json::Value) -> Vec<String> {
    let mut keys = vec![];
    if let Some(name) = project["name"].as_str() {
        keys.push(name.to_lowercase());
        keys.push(
            name.to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("-"),
        );
    }
    if let Some(slug) = project["slug_id"].as_str() {
        keys.push(slug.to_lowercase());
    }
    if let Some(url) = project["url"].as_str()
        && let Some((_, path)) = url.split_once("/project/")
        && let Some(slug) = path.split('/').next()
    {
        keys.push(slug.to_lowercase());
    }
    keys
}
pub fn catalog(kind: &str) -> Result<(&'static str, &'static str)> {
    match kind {
        "team" => Ok(("Teams", "teams")),
        "user" => Ok(("Users", "users")),
        "state" => Ok(("States", "workflowStates")),
        "label" => Ok(("Labels", "issueLabels")),
        "project" => Ok(("Projects", "projects")),
        _ => Err(AppError::input("Unsupported reference type")),
    }
}
pub fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.chars().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

pub fn label_scope(team: &str) -> serde_json::Value {
    json!({"or":[{"team":{"id":{"eq":team}}},{"team":{"null":true}}]})
}
