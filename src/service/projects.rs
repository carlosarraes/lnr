use super::{
    Service,
    issues::IssueFilter,
    page::{PageRequest, Reply},
};
use crate::error::{AppError, Result};
use serde_json::json;
#[derive(Default)]
pub struct ProjectFilter {
    pub team: Option<String>,
    pub state: Option<String>,
    pub involvement: Vec<String>,
}
impl Service {
    pub async fn list_projects(
        &self,
        f: ProjectFilter,
        page: PageRequest,
    ) -> Result<Reply<Vec<crate::model::Project>>> {
        let mut filter = json!({});
        if let Some(team) = f.team {
            filter["accessibleTeams"] =
                json!({"some":{"id":{"eq":self.resolve("team",&team,None).await?}}});
        }
        if let Some(state) = f.state {
            filter["status"] = json!({"name":{"eq":state}});
        }
        if !f.involvement.is_empty() {
            let mut alternatives = vec![];
            for kind in f.involvement {
                alternatives.push(match kind.as_str() {
                    "lead" => json!({"lead":{"isMe":{"eq":true}}}),
                    "member" => json!({"members":{"some":{"isMe":{"eq":true}}}}),
                    "assignee" => json!({"issues":{"some":{"assignee":{"isMe":{"eq":true}}}}}),
                    _ => return Err(AppError::input("Unknown involvement type")),
                });
            }
            filter["or"] = alternatives.into();
        }
        self.list("Projects", "projects", filter, page)
            .await?
            .decode()
    }
    pub async fn view_project(&self, reference: &str) -> Result<crate::model::Project> {
        let id = self.resolve("project", reference, None).await?;
        let data = self.query("ProjectView", json!({"id":id})).await?;
        crate::model::decode(
            data.get("project")
                .filter(|v| !v.is_null())
                .cloned()
                .ok_or_else(|| AppError::new("not_found", "Project not found"))?,
        )
    }
    pub async fn project_issues(
        &self,
        reference: &str,
        mut filter: IssueFilter,
        page: PageRequest,
    ) -> Result<Reply<Vec<crate::model::Issue>>> {
        if filter.project.is_some() {
            return Err(AppError::input(
                "Project is already specified by the positional argument",
            ));
        }
        filter.project = Some(reference.into());
        self.list_issues(filter, page).await
    }
}
