use super::args::Cli;
use clap::CommandFactory;
use schemars::JsonSchema;
use serde_json::{Value, json};
#[derive(JsonSchema)]
#[allow(dead_code)]
struct EnvelopeSchema {
    schema_version: u8,
    data: Value,
    meta: Value,
    error: Option<ErrorSchema>,
}
#[derive(JsonSchema)]
#[allow(dead_code)]
struct ErrorSchema {
    code: String,
    message: String,
    retryable: bool,
    details: Value,
}
pub fn command_schema() -> Value {
    let mut cli = Cli::command();
    cli.build();
    let mut commands = vec![];
    walk(&cli, "lnr", &mut commands);
    json!({"commands":commands,"output_schema":schemars::schema_for!(EnvelopeSchema),"entities":{"issue":schemars::schema_for!(crate::model::Issue),"project":schemars::schema_for!(crate::model::Project),"comment":schemars::schema_for!(crate::model::Comment),"relation":schemars::schema_for!(crate::model::Relation)},"exit_codes":{"0":"success","2":"invalid_input","3":"authentication","4":"not_found","5":"ambiguous","6":"rate_limited","7":"network","8":"api_or_uncertain_mutation","9":"io_or_configuration"},"pagination":{"default_limit":50,"max_limit":250,"context_default_limit":20,"context_max_limit":100},"output":{"piped":"json","terminal":"text","schema_version":1}})
}
fn walk(command: &clap::Command, path: &str, result: &mut Vec<Value>) {
    if !command.has_subcommands() {
        result.push(json!({"path":path,"about":command.get_about().map(ToString::to_string),"arguments":command.get_arguments().map(|a|json!({"name":a.get_id().as_str(),"long":a.get_long(),"required":a.is_required_set(),"global":a.is_global_set(),"action":format!("{:?}",a.get_action()),"defaults":a.get_default_values().iter().map(|s|s.to_string_lossy()).collect::<Vec<_>>(),"possible_values":a.get_value_parser().possible_values().map(|v|v.map(|p|p.get_name().to_owned()).collect::<Vec<_>>())})).collect::<Vec<_>>()}));
    }
    for sub in command.get_subcommands() {
        if sub.get_name() != "help" {
            walk(sub, &format!("{path} {}", sub.get_name()), result);
        }
    }
}
