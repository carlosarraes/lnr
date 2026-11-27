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
    pub title: Option<String>,
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
        let filter = self.issue_filter(f).await?;
        self.list("Issues", "issues", filter, page).await?.decode()
    }
    pub(crate) async fn issue_filter(&self, f: IssueFilter) -> Result<Value> {
        if f.title.as_ref().is_some_and(|s| s.trim().is_empty()) {
            return Err(AppError::input("Title filter must not be empty"));
        }
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
        if let Some(title) = f.title {
            filter["title"] = json!({"containsIgnoreCase":title});
        }
        Ok(filter)
    }
    /// Fetch the complete discussion plus a first page of relations in both directions.
    pub async fn inspect_issue(
        &self,
        reference: &str,
        comments: bool,
    ) -> Result<Reply<crate::model::IssueView>> {
        if reference.trim().is_empty() {
            return Err(AppError::input("Issue reference must not be empty"));
        }
        let result = self
            .query(
                "IssueView",
                json!({"id":reference,"includeComments":comments,"includeRelations":true}),
            )
            .await;
        let (mut data, mut error) = split(result);
        if data["issue"].is_null() {
            return Err(error.unwrap_or_else(|| AppError::new("not_found", "Issue not found")));
        }
        let raw = data["issue"].take();
        let mut issue = crate::model::normalize(raw.clone());
        crate::model::flatten_labels(&mut issue);
        if let Some(map) = issue.as_object_mut() {
            map.remove("inverse_relations");
            map.remove("comments");
        }
        let mut collections = json!({});
        let mut complete = true;
        let mut directions = vec![];
        for (field, output) in [
            ("relations", "relations"),
            ("inverseRelations", "inverse_relations"),
        ] {
            let mut pages = Pages::new(None);
            if let Err(e) = pages.ingest(&raw[field])
                && error.is_none()
            {
                error = Some(e);
            }
            complete &= pages.meta["complete"] == true;
            collections[output] = pages.meta;
            directions.push(Value::from(pages.items));
        }
        let incoming = directions.pop().unwrap_or_default();
        let outgoing = directions.pop().unwrap_or_default();
        match (
            crate::model::decode(outgoing),
            crate::model::decode(incoming),
        ) {
            (Ok(outgoing), Ok(incoming)) => {
                issue["relations"] =
                    serde_json::to_value(crate::model::RelationGroups::new(outgoing, incoming))
                        .expect("JSON-compatible relations");
            }
            (Err(e), _) | (_, Err(e)) => {
                issue["relations"] = Value::Null;
                error.get_or_insert(e);
            }
        }
        if comments {
            let mut pages = Pages::new(None);
            let mut connection = raw["comments"].clone();
            loop {
                if let Err(e) = pages.ingest(&connection)
                    && error.is_none()
                {
                    error = Some(e);
                }
                if error.is_some() || !pages.has_more() {
                    break;
                }
                let (data, e) = split(
                    self.query(
                        "Comments",
                        json!({"id":reference,"first":100,"after":pages.cursor}),
                    )
                    .await,
                );
                error = e;
                connection = data["issue"]["comments"].clone();
            }
            // Linear timestamps are UTC ISO-8601. Tie-break equal timestamps by ID.
            pages.items.sort_by(|a, b| {
                a["created_at"]
                    .as_str()
                    .cmp(&b["created_at"].as_str())
                    .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
            });
            issue["comments"] = pages.items.into();
            if error.is_some() {
                pages.meta["complete"] = false.into();
            }
            complete &= pages.meta["complete"] == true;
            collections["comments"] = pages.meta;
        }
        let meta = json!({"complete":complete && error.is_none(),"collections":collections});
        if let Some(mut e) = error {
            e.data = Some(issue);
            e.meta = Box::new(meta);
            return Err(e);
        }
        Reply { data: issue, meta }.decode()
    }
    pub async fn view_issue(&self, reference: &str) -> Result<crate::model::Issue> {
        if reference.trim().is_empty() {
            return Err(AppError::input("Issue reference must not be empty"));
        }
        let data = self.query("IssueView", json!({"id":reference})).await?;
        let mut issue = data
            .get("issue")
            .filter(|v| !v.is_null())
            .cloned()
            .ok_or_else(|| AppError::new("not_found", "Issue not found"))?;
        crate::model::flatten_labels(&mut issue);
        crate::model::decode(issue)
    }
}

fn split(result: Result<Value>) -> (Value, Option<AppError>) {
    match result {
        Ok(data) => (data, None),
        Err(mut e) => (e.data.take().unwrap_or(Value::Null), Some(e)),
    }
}
