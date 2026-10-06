#![allow(dead_code, clippy::upper_case_acronyms)]
use graphql_client::GraphQLQuery;
type DateTime = String;
type TimelessDate = String;
type Duration = String;
type DateTimeOrDuration = String;
type TimelessDateOrDuration = String;
type UUID = String;
type JSON = serde_json::Value;
type JSONObject = serde_json::Value;
macro_rules! checked {
 ($($name:ident),*) => {$(
 #[derive(GraphQLQuery)]
 #[graphql(schema_path="graphql/schema.graphql",query_path="graphql/operations.graphql",response_derives="Debug",variables_derives="Clone")]
 pub struct $name;
 )*};
}
checked!(
    AuthStatus,
    Issues,
    IssueView,
    Projects,
    ProjectView,
    Teams,
    Users,
    States,
    Labels,
    IssueContext,
    Comments,
    Relations,
    CreateIssue,
    UpdateIssue,
    AddComment,
    AddRelation,
    RemoveRelation
);
pub const DOCUMENT: &str = include_str!("../../graphql/operations.graphql");
