use super::{Service, page::PageRequest};
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
