//! Project overview reads, kept separate from the agent CLI response contract.
use super::{
    Service,
    page::{PageRequest, Pages, Reply},
};
use crate::{
    error::{AppError, Result},
    model::{AuthStatus, Entity, Project},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OverviewFilter {
    pub mine: bool,
    pub team_ids: Vec<String>,
    pub status_ids: Vec<String>,
    pub involvement: Vec<String>,
    pub query: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewProject {
    pub project: Project,
    pub teams: Vec<Entity>,
    pub involvement: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct OverviewCatalog {
    pub viewer: AuthStatus,
    pub teams: Vec<Entity>,
    pub statuses: Vec<Entity>,
}
fn involvement(kinds: &[String]) -> Result<Value> {
    let alternatives: Result<Vec<Value>> = kinds
        .iter()
        .map(|kind| {
            Ok(match kind.as_str() {
                "lead" => json!({"lead":{"isMe":{"eq":true}}}),
                "member" => json!({"members":{"some":{"isMe":{"eq":true}}}}),
                "assignee" => json!({"issues":{"some":{"assignee":{"isMe":{"eq":true}}}}}),
                _ => return Err(AppError::input("Unknown involvement type")),
            })
        })
        .collect();
    Ok(json!({"or":alternatives?}))
}
impl Service {
    pub async fn overview_catalog(&self) -> Result<OverviewCatalog> {
        let viewer = self.status().await?;
        let teams = self
            .list(
                "Teams",
                "teams",
                json!({}),
                PageRequest {
                    all: true,
                    ..Default::default()
                },
            )
            .await?
            .decode::<Vec<Entity>>()?
            .data;
        let statuses = self
            .list(
                "OverviewStatuses",
                "projectStatuses",
                json!({}),
                PageRequest {
                    all: true,
                    ..Default::default()
                },
            )
            .await?
            .decode::<Vec<Entity>>()?
            .data;
        Ok(OverviewCatalog {
            viewer,
            teams,
            statuses,
        })
    }
    pub async fn overview_projects(
        &self,
        f: OverviewFilter,
        page: PageRequest,
    ) -> Result<Reply<Vec<OverviewProject>>> {
        let mut filter = json!({});
        if !f.team_ids.is_empty() {
            filter["accessibleTeams"] = json!({"some":{"id":{"in":f.team_ids}}});
        }
        if !f.status_ids.is_empty() {
            filter["status"] = json!({"id":{"in":f.status_ids}});
        }
        if !f.query.is_empty() {
            filter["name"] = json!({"containsIgnoreCase":f.query});
        }
        let mut groups = vec![];
        if f.mine {
            groups.push(involvement(&[
                "lead".into(),
                "member".into(),
                "assignee".into(),
            ])?);
        }
        if !f.involvement.is_empty() {
            groups.push(involvement(&f.involvement)?);
        }
        if !groups.is_empty() {
            filter["and"] = groups.into();
        }
        let reply = self
            .list("OverviewProjects", "projects", filter, page)
            .await?;
        let mut result = vec![];
        for node in reply
            .data
            .as_array()
            .ok_or_else(|| AppError::new("api", "Missing project nodes"))?
        {
            let mut roles = vec![];
            if node["lead"]["is_me"] == true {
                roles.push("lead".into());
            }
            for (key, role) in [("members", "member"), ("issues", "assignee")] {
                let nodes = node[key]["nodes"]
                    .as_array()
                    .ok_or_else(|| AppError::new("api", "Missing project involvement"))?;
                if !nodes.is_empty() {
                    roles.push(role.into());
                }
            }
            let project: Project = crate::model::decode(node.clone())?;
            // list() normalizes camelCase recursively; Pages consumes original API keys.
            let teams_node = &node["teams"];
            let mut teams: Vec<Entity> = crate::model::decode(teams_node["nodes"].clone())?;
            if teams_node["page_info"]["has_next_page"] == true {
                let cursor = teams_node["page_info"]["end_cursor"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| AppError::new("api", "Missing project team cursor"))?;
                let mut pages = Pages::new(Some(cursor.into()));
                loop {
                    let data = self
                        .query(
                            "OverviewTeams",
                            json!({"id":project.id,"first":250,"after":pages.cursor}),
                        )
                        .await?;
                    pages.ingest(&data["project"]["teams"])?;
                    if !pages.has_more() {
                        break;
                    }
                }
                teams.extend(pages.finish().decode::<Vec<Entity>>()?.data);
            } else if teams_node["page_info"]["has_next_page"] != false {
                return Err(AppError::new(
                    "api",
                    "Missing project team page information",
                ));
            }
            result.push(OverviewProject {
                project,
                teams,
                involvement: roles,
            });
        }
        Ok(Reply {
            data: result,
            meta: reply.meta,
        })
    }
}
