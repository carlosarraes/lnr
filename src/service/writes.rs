use super::Service;
use crate::{
    api::OperationKind,
    error::{AppError, Result},
};
use serde_json::{Value, json};
#[derive(Default, Clone, Debug)]
pub enum Patch<T> {
    #[default]
    Keep,
    Set(T),
    Clear,
}
#[derive(Default, Clone, Debug)]
pub struct IssuePatch {
    pub title: Option<String>,
    pub state: Option<String>,
    pub assignee: Patch<String>,
    pub project: Patch<String>,
    pub parent: Patch<String>,
    pub labels: Patch<Vec<String>>,
    pub priority: Option<u8>,
    pub description: Patch<String>,
}
impl IssuePatch {
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.state.is_none()
            && self.priority.is_none()
            && matches!(self.assignee, Patch::Keep)
            && matches!(self.project, Patch::Keep)
            && matches!(self.parent, Patch::Keep)
            && matches!(self.labels, Patch::Keep)
            && matches!(self.description, Patch::Keep)
    }
}
impl Service {
    pub(crate) async fn mutate(
        &self,
        op: &str,
        root: &str,
        entity: Option<&str>,
        vars: Value,
    ) -> Result<Value> {
        let data = self
            .client
            .execute(
                crate::api::document::selected(op)?,
                vars,
                Some(op),
                OperationKind::Mutation,
            )
            .await?;
        let payload = &data[root];
        if payload["success"].as_bool() != Some(true) {
            return Err(AppError::new(
                if payload["success"].as_bool() == Some(false) {
                    "mutation_rejected"
                } else {
                    "uncertain_mutation"
                },
                "Mutation did not report success; check current state before retrying",
            ));
        }
        match entity {
            Some(key) => payload
                .get(key)
                .filter(|v| !v.is_null())
                .cloned()
                .ok_or_else(|| {
                    AppError::new(
                        "uncertain_mutation",
                        "Mutation returned no entity; check current state before retrying",
                    )
                }),
            None => Ok(json!({"success":true})),
        }
    }
    pub async fn create_issue(
        &self,
        team: &str,
        title: String,
        fields: IssuePatch,
    ) -> Result<crate::model::Issue> {
        if title.trim().is_empty() {
            return Err(AppError::input("Title must not be empty"));
        }
        let team = self.resolve("team", team, None).await?;
        let mut input = self.write_input(fields, Some(&team)).await?;
        input["teamId"] = team.into();
        input["title"] = title.into();
        crate::model::decode(
            self.mutate(
                "CreateIssue",
                "issueCreate",
                Some("issue"),
                json!({"input":input}),
            )
            .await?,
        )
    }
    pub async fn update_issue(
        &self,
        reference: &str,
        fields: IssuePatch,
    ) -> Result<crate::model::Issue> {
        if fields.is_empty() {
            return Err(AppError::input("Specify at least one field to update"));
        }
        let needs_team = fields.state.is_some() || matches!(fields.labels, Patch::Set(_));
        let issue = if needs_team {
            Some(self.view_issue(reference).await?)
        } else {
            None
        };
        let team = if needs_team {
            Some(
                issue
                    .as_ref()
                    .and_then(|i| i.team.as_ref())
                    .map(|t| t.id.as_str())
                    .ok_or_else(|| AppError::new("api", "Issue has no team"))?,
            )
        } else {
            None
        };
        let input = self.write_input(fields, team).await?;
        crate::model::decode(
            self.mutate(
                "UpdateIssue",
                "issueUpdate",
                Some("issue"),
                json!({"id":reference,"input":input}),
            )
            .await?,
        )
    }
    async fn write_input(&self, p: IssuePatch, team: Option<&str>) -> Result<Value> {
        let mut input = json!({});
        if let Some(title) = p.title {
            if title.trim().is_empty() {
                return Err(AppError::input("Title must not be empty"));
            }
            input["title"] = title.into();
        }
        if let Some(priority) = p.priority {
            if priority > 4 {
                return Err(AppError::input("Priority must be 0..4"));
            }
            input["priority"] = priority.into();
        }
        if let Some(state) = p.state {
            input["stateId"] = self.resolve("state", &state, team).await?.into();
        }
        for (field, kind, patch) in [
            ("assigneeId", "user", p.assignee),
            ("projectId", "project", p.project),
            ("parentId", "issue", p.parent),
        ] {
            match patch {
                Patch::Keep => {}
                Patch::Clear => input[field] = Value::Null,
                Patch::Set(s) => input[field] = self.resolve(kind, &s, None).await?.into(),
            }
        }
        match p.description {
            Patch::Keep => {}
            Patch::Clear => input["description"] = Value::Null,
            Patch::Set(s) => input["description"] = s.into(),
        }
        match p.labels {
            Patch::Keep => {}
            Patch::Clear => input["labelIds"] = json!([]),
            Patch::Set(labels) => {
                let mut ids = vec![];
                for label in labels {
                    ids.push(self.resolve("label", &label, team).await?);
                }
                input["labelIds"] = json!(ids);
            }
        }
        Ok(input)
    }
    pub async fn add_comment(&self, reference: &str, body: String) -> Result<Value> {
        if body.trim().is_empty() {
            return Err(AppError::input("Comment body must not be empty"));
        }
        let id = self.resolve("issue", reference, None).await?;
        self.mutate(
            "AddComment",
            "commentCreate",
            Some("comment"),
            json!({"input":{"issueId":id,"body":body}}),
        )
        .await
    }
    pub async fn add_relation(&self, source: &str, target: &str, kind: &str) -> Result<Value> {
        if !["blocks", "related", "duplicate"].contains(&kind) {
            return Err(AppError::input("Unsupported relation type"));
        }
        let source = self.resolve("issue", source, None).await?;
        let target = self.resolve("issue", target, None).await?;
        if source == target {
            return Err(AppError::input("An issue cannot relate to itself"));
        }
        self.mutate(
            "AddRelation",
            "issueRelationCreate",
            Some("issueRelation"),
            json!({"input":{"issueId":source,"relatedIssueId":target,"type":kind}}),
        )
        .await
    }
    pub async fn remove_relation(&self, id: &str) -> Result<Value> {
        if !super::resolve::is_uuid(id) {
            return Err(AppError::input("Relation removal requires a UUID"));
        }
        self.mutate(
            "RemoveRelation",
            "issueRelationDelete",
            None,
            json!({"id":id}),
        )
        .await
    }
}
