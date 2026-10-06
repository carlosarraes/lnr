use super::{
    Service,
    page::{PageRequest, Reply, connection},
};
use crate::{
    api::OperationKind,
    error::{AppError, Result},
};
use serde_json::{Value, json};
use std::collections::HashSet;
#[derive(Debug, Default, Clone)]
pub struct IssueFilter {
    pub team: Option<String>,
    pub assignee: Option<String>,
    pub project: Option<String>,
    pub state: Option<String>,
    pub search: Option<String>,
}
impl Service {
    pub(crate) async fn query(&self, op: &str, variables: Value) -> Result<Value> {
        self.client
            .execute(
                crate::api::document::selected(op)?,
                variables,
                Some(op),
                OperationKind::Query,
            )
            .await
    }
    pub async fn list(
        &self,
        op: &str,
        root: &str,
        filter: Value,
        page: PageRequest,
    ) -> Result<Reply> {
        if !(1..=250).contains(&page.limit) {
            return Err(AppError::input("Page limit must be 1..250"));
        }
        let mut after = page.after;
        let mut seen = HashSet::new();
        if let Some(c) = &after {
            seen.insert(c.clone());
        }
        let mut items = vec![];
        loop {
            let result = self
                .query(
                    op,
                    json!({"filter":filter,"first":page.limit,"after":after}),
                )
                .await;
            let data = match result {
                Ok(v) => v,
                Err(mut e) => {
                    if !items.is_empty() {
                        e.data = Some(Value::Array(items));
                    }
                    return Err(e);
                }
            };
            let (nodes, meta) = match connection(&data[root]) {
                Ok(v) => v,
                Err(mut e) => {
                    e.data = Some(Value::Array(items));
                    return Err(e);
                }
            };
            items.extend(nodes);
            let more = meta["has_more"].as_bool().unwrap_or(false);
            if !page.all || !more {
                return Ok(Reply {
                    data: Value::Array(items),
                    meta: json!({"page":meta,"complete":!more}),
                });
            }
            after = meta["end_cursor"].as_str().map(str::to_owned);
            if !seen.insert(after.clone().unwrap_or_default()) {
                let mut e = AppError::new("api", "Pagination cursor repeated");
                e.data = Some(Value::Array(items));
                return Err(e);
            }
        }
    }
    pub async fn list_issues(&self, f: IssueFilter, page: PageRequest) -> Result<Reply> {
        let mut filter = json!({});
        let mut team_id = None;
        if let Some(team) = f.team {
            let id = self.resolve("team", &team, None).await?;
            filter["team"] = json!({"id":{"eq":id}});
            team_id = Some(id);
        }
        if let Some(assignee) = f.assignee {
            filter["assignee"] = if assignee == "me" {
                json!({"isMe":{"eq":true}})
            } else {
                json!({"id":{"eq":self.resolve("user",&assignee,None).await?}})
            };
        }
        if let Some(project) = f.project {
            filter["project"] = json!({"id":{"eq":self.resolve("project",&project,None).await?}});
        }
        if let Some(state) = f.state {
            filter["state"] = if [
                "triage",
                "backlog",
                "unstarted",
                "started",
                "completed",
                "canceled",
            ]
            .contains(&state.as_str())
            {
                json!({"type":{"eq":state}})
            } else {
                let team = team_id.as_deref().ok_or_else(|| {
                    AppError::input("State names require --team; state types work across teams")
                })?;
                json!({"id":{"eq":self.resolve("state",&state,Some(team)).await?}})
            };
        }
        if let Some(search) = f.search {
            if search.trim().is_empty() {
                return Err(AppError::input("Search must not be empty"));
            }
            filter["searchableContent"] = json!({"contains":search});
        }
        self.list("Issues", "issues", filter, page).await
    }
    pub async fn view_issue(&self, reference: &str) -> Result<crate::model::Issue> {
        if reference.trim().is_empty() {
            return Err(AppError::input("Issue reference must not be empty"));
        }
        let data = self.query("IssueView", json!({"id":reference})).await?;
        crate::model::decode(
            data.get("issue")
                .filter(|v| !v.is_null())
                .cloned()
                .ok_or_else(|| AppError::new("not_found", "Issue not found"))?,
        )
    }
}
