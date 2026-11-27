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
    #[serde(default, alias = "branchName")]
    pub branch_name: Option<String>,
    #[serde(default)]
    pub creator: Option<Entity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<Entity>>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentioned_users: Vec<MentionedUser>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MentionedUser {
    pub id: String,
    pub name: String,
    pub url: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IssueContext {
    #[serde(flatten)]
    pub issue: Issue,
    pub children: Vec<Issue>,
    pub comments: Vec<Comment>,
    pub relations: Vec<Relation>,
    pub inverse_relations: Vec<Relation>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RelationsPage {
    pub relations: Vec<Relation>,
    pub inverse_relations: Vec<Relation>,
}
/// Replace a `{nodes: [...]}` connection with its node list.
pub fn flatten_labels(issue: &mut serde_json::Value) {
    if let Some(nodes) = issue.pointer("/labels/nodes").cloned() {
        issue["labels"] = nodes;
    }
}
/// Normalize built-in response keys; raw GraphQL deliberately bypasses this adapter.
pub fn normalize(value: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(normalize).collect()),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let mut normalized = String::new();
                    for c in key.chars() {
                        if c.is_ascii_uppercase() {
                            normalized.push('_');
                            normalized.push(c.to_ascii_lowercase());
                        } else {
                            normalized.push(c);
                        }
                    }
                    (normalized, normalize(value))
                })
                .collect(),
        ),
        other => other,
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Viewer {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Organization {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(alias = "urlKey")]
    pub url_key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AuthStatus {
    pub viewer: Viewer,
    #[serde(default)]
    pub organization: Option<Organization>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IssueView {
    #[serde(flatten)]
    pub issue: Issue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comments: Option<Vec<Comment>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relations: Option<RelationGroups>,
}
/// Relations seen from one issue. `blocks`, `related` and `duplicate_of` keep
/// outgoing relations (the other issue is `related_issue`); `blocked_by` and
/// `duplicated_by` hold incoming ones (the other issue is `issue`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RelationGroups {
    pub blocks: Vec<Relation>,
    pub blocked_by: Vec<Relation>,
    pub related: Vec<Relation>,
    pub duplicate_of: Vec<Relation>,
    pub duplicated_by: Vec<Relation>,
    pub other: Vec<Relation>,
}
impl RelationGroups {
    pub fn new(outgoing: Vec<Relation>, incoming: Vec<Relation>) -> Self {
        let mut groups = Self::default();
        for (relation, inverse) in outgoing
            .into_iter()
            .map(|r| (r, false))
            .chain(incoming.into_iter().map(|r| (r, true)))
        {
            let group = match (relation.kind.as_deref(), inverse) {
                (Some("blocks"), false) => &mut groups.blocks,
                (Some("blocks"), true) => &mut groups.blocked_by,
                (Some("related"), _) => &mut groups.related,
                (Some("duplicate"), false) => &mut groups.duplicate_of,
                (Some("duplicate"), true) => &mut groups.duplicated_by,
                _ => &mut groups.other,
            };
            group.push(relation);
        }
        groups
    }
}
