//! Stable data returned by workflows; no terminal dependencies.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Entity {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Issue {
    pub id: String,
    #[serde(default)]
    pub identifier: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub priority: Option<f64>,
    #[serde(default)]
    pub state: Option<Entity>,
    #[serde(default)]
    pub team: Option<Entity>,
    #[serde(default)]
    pub assignee: Option<Entity>,
    #[serde(default)]
    pub project: Option<Entity>,
    #[serde(default)]
    pub parent: Option<IssueRef>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IssueRef {
    pub id: String,
    #[serde(default)]
    pub identifier: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub progress: Option<f64>,
    #[serde(default, alias = "targetDate")]
    pub target_date: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub status: Option<Entity>,
    #[serde(default)]
    pub lead: Option<Entity>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Comment {
    pub id: String,
    pub body: String,
    #[serde(default, alias = "createdAt")]
    pub created_at: Option<String>,
    #[serde(default)]
    pub user: Option<Entity>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Relation {
    pub id: String,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub issue: Option<IssueRef>,
    #[serde(default, alias = "relatedIssue")]
    pub related_issue: Option<IssueRef>,
}
pub fn decode<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> crate::error::Result<T> {
    serde_json::from_value(value).map_err(|_| {
        crate::error::AppError::new("api", "API data does not match the expected entity shape")
    })
}
