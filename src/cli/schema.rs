use super::args::Cli;
use crate::model;
use clap::CommandFactory;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Serialize, JsonSchema)]
struct EnvelopeSchema<T> {
    #[schemars(range(min = 1, max = 1))]
    schema_version: u8,
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorSchema>,
    meta: ResponseMeta,
}
#[derive(Serialize, JsonSchema)]
struct ErrorSchema {
    code: String,
    message: String,
    retryable: bool,
    details: Value,
}
#[derive(Serialize, JsonSchema)]
struct PageSchema {
    end_cursor: Option<String>,
    has_more: Option<bool>,
    complete: bool,
}
#[derive(Serialize, JsonSchema)]
struct ResponseMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    complete: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<PageSchema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    collections: Option<BTreeMap<String, PageSchema>>,
}
#[derive(Serialize, JsonSchema)]
struct CatalogEntity {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
}
fn envelope<T: JsonSchema>() -> Value {
    serde_json::to_value(schemars::schema_for!(EnvelopeSchema<T>)).expect("schema serializes")
}
pub fn command_schema() -> Value {
    let mut cli = Cli::command();
    cli.build();
    let mut commands = vec![];
    walk(&cli, "lnr", &mut commands);
    let outputs = json!({
     "issue_view":envelope::<model::IssueView>(),"issue":envelope::<model::Issue>(),"issues":envelope::<Vec<model::Issue>>(),
     "issue_context":envelope::<model::IssueContext>(),"project":envelope::<model::Project>(),"projects":envelope::<Vec<model::Project>>(),
     "comment":envelope::<model::Comment>(),"comments":envelope::<Vec<model::Comment>>(),"relation":envelope::<model::Relation>(),"relations":envelope::<model::RelationsPage>(),
     "catalog":envelope::<Vec<CatalogEntity>>(),"auth":envelope::<crate::auth::AuthResponse>(),"receipt":envelope::<Value>(),"raw":envelope::<Value>(),"discovery":envelope::<Value>()
    });
    json!({"commands":commands,"output_schema":envelope::<Value>(),"outputs":outputs,"error_output_schema":envelope::<Value>(),
 "entities":{"issue":schemars::schema_for!(model::Issue),"issue_context":schemars::schema_for!(model::IssueContext),"project":schemars::schema_for!(model::Project),"comment":schemars::schema_for!(model::Comment),"relation":schemars::schema_for!(model::Relation)},
 "exit_codes":{"0":"success","2":"invalid_input","3":"authentication","4":"not_found","5":"ambiguous","6":"rate_limited","7":"network","8":"api_or_uncertain_mutation","9":"io_or_configuration"},
 "pagination":{"default_limit":50,"max_limit":250,"context_default_limit":20,"context_max_limit":100},"output":{"piped":"json","terminal":"text","schema_version":1},
 "partial_failures":"Partial data can omit unavailable fields; error_output_schema applies when exit status is nonzero. Collection metadata marks unavailable or incomplete collections."})
}
fn output_name(path: &str) -> &'static str {
    match path {
        "lnr issue view" => "issue_view",
        "lnr issue create" | "lnr issue update" => "issue",
        "lnr issue list" | "lnr project issues" => "issues",
        "lnr issue context" => "issue_context",
        "lnr issue comment list" => "comments",
        "lnr issue comment add" => "comment",
        "lnr issue relation list" => "relations",
        "lnr issue relation add" => "relation",
        "lnr project list" => "projects",
        "lnr project view" => "project",
        "lnr team list" | "lnr user list" | "lnr state list" | "lnr label list" => "catalog",
        "lnr auth status" => "auth",
        "lnr api" => "raw",
        "lnr schema" => "discovery",
        _ => "receipt",
    }
}
fn walk(command: &clap::Command, path: &str, result: &mut Vec<Value>) {
    if !command.has_subcommands() {
        let mut constraints = json!({});
        if path == "lnr issue relation add" {
            constraints["exactly_one"] = json!(["blocks", "related", "duplicate_of"]);
        }
        if path == "lnr auth import-linear" {
            constraints["required"] = json!(["workspace"]);
        }
        if path == "lnr api" {
            constraints["stdin_sources_max"] = 1.into();
            constraints["operation_name_required_when"] =
                json!("Document has more than one operation");
        }
        if path == "lnr issue list" || path == "lnr project issues" {
            constraints["team_required_when"] =
                json!("--state is a name rather than a UUID or state type");
        }
        if path == "lnr issue update" {
            constraints["at_least_one"] = json!([
                "title",
                "state",
                "assignee",
                "unassign",
                "project",
                "clear_project",
                "parent",
                "clear_parent",
                "labels",
                "clear_labels",
                "priority",
                "description_file",
                "clear_description"
            ]);
        }
        let arguments=command.get_arguments().map(|a|{
   let required=a.is_required_set()||(path=="lnr auth import-linear" && a.get_id()=="workspace");
   let mut v=json!({"name":a.get_id().as_str(),"long":a.get_long(),"required":required,"global":a.is_global_set(),"help":a.get_help().map(ToString::to_string),"action":format!("{:?}",a.get_action()),"defaults":a.get_default_values().iter().map(|s|s.to_string_lossy()).collect::<Vec<_>>(),"possible_values":a.get_value_parser().possible_values().map(|v|v.map(|p|p.get_name().to_owned()).collect::<Vec<_>>()),"conflicts":command.get_arg_conflicts_with(a).iter().map(|a|a.get_id().as_str()).collect::<Vec<_>>()});
   match a.get_id().as_str(){"limit"=>{v["minimum"]=1.into();v["maximum"]=if path=="lnr issue context"{100.into()}else{250.into()};v["effective_default"]=if path=="lnr issue context"{20.into()}else{50.into()};},"priority"=>{v["minimum"]=0.into();v["maximum"]=4.into();},_=>{}}
   v
  }).collect::<Vec<_>>();
        result.push(json!({"path":path,"about":command.get_about().map(ToString::to_string),"arguments":arguments,"constraints":constraints,"output":output_name(path)}));
    }
    for sub in command.get_subcommands() {
        if sub.get_name() != "help" {
            walk(sub, &format!("{path} {}", sub.get_name()), result);
        }
    }
}
