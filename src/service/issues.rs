use super::{
    Service,
    page::{PageRequest, Pages, Reply},
};
use crate::{
    api::OperationKind,
    error::{AppError, Result},
};
use serde_json::{Value, json};

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
        page.validate()?;
        let mut pages = Pages::new(page.after);
        loop {
            let result = self
                .query(
                    op,
                    json!({"filter":filter,"first":page.limit,"after":pages.cursor}),
                )
                .await;
            let data = match result {
                Ok(v) => v,
                Err(e) => {
                    if let Some(partial) = &e.data {
                        let _ = pages.ingest(&partial[root]);
                    }
                    return Err(pages.fail(e));
                }
            };
            if let Err(e) = pages.ingest(&data[root]) {
                return Err(pages.fail(e));
            }
            if !page.all || !pages.has_more() {
                return Ok(pages.finish());
            }
        }
    }
    pub async fn list_issues(
        &self,
        f: IssueFilter,
        page: PageRequest,
    ) -> Result<Reply<Vec<crate::model::Issue>>> {
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
            } else if super::resolve::is_uuid(&state) {
                json!({"id":{"eq":state}})
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
        self.list("Issues", "issues", filter, page).await?.decode()
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
