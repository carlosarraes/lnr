pub mod args;
use crate::error::AppError;
use args::Cli;
use clap::{CommandFactory, Parser};
use serde_json::json;
use std::io::{IsTerminal, Write};

pub async fn run() -> i32 {
    let args: Vec<String> = std::env::args().collect();
    let json_mode = args.iter().any(|x| x=="--json" || x=="--output=json") || args.windows(2).any(|x|x[0]=="--output" && x[1]=="json") || (!std::io::stdout().is_terminal() && !args.iter().any(|x|x=="text" || x=="--output=text"));
    let cli = match Cli::try_parse_from(&args) {
        Ok(c) => c,
        Err(e) if matches!(e.kind(), clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion) => { print!("{e}"); return 0; }
        Err(e) => return render_error(AppError::input(e.to_string()), json_mode),
    };
    if cli.command.is_none() { let _ = Cli::command().print_help(); println!(); return 0; }
    match dispatch(cli).await {
        Ok(value) => { if json_mode { println!("{}",json!({"schema_version":1,"data":value.data,"meta":value.meta})); } else {println!("{}",serde_json::to_string_pretty(&value.data).unwrap_or_default());} 0 }
        Err(e) => render_error(e,json_mode),
    }
}
fn render_error(error: AppError, json_mode: bool) -> i32 {
    let code=error.exit_code();
    if json_mode {
        let value=json!({"schema_version":1,"data":error.data,"error":error,"meta":{"complete":false}});
        let _=writeln!(std::io::stdout(),"{value}");
    } else { eprintln!("{}: {}",error.code,error.message); }
    code
}

pub fn read_body(path: &str) -> crate::error::Result<String> {
    use std::io::Read;
    if path == "-" {let mut s=String::new();std::io::stdin().read_to_string(&mut s)?;Ok(s)}else{Ok(std::fs::read_to_string(path)?)}
}
async fn dispatch(cli: Cli) -> crate::error::Result<crate::service::page::Reply> {
    use args::{Command,AuthCommand};
    if let Some(Command::Auth{command:AuthCommand::ImportLinear{replace}})=&cli.command {
        let workspace=cli.workspace.as_deref().ok_or_else(||AppError::input("auth import-linear requires --workspace"))?;
        return crate::auth::import::import_linear(workspace,*replace).await.map(crate::service::page::Reply::data);
    }
    let service=crate::service::Service::new(crate::api::ApiClient::new(crate::auth::resolve(cli.workspace.as_deref())?)?);
    match cli.command.unwrap() {
        Command::Auth{command:AuthCommand::Status}=>service.status().await.map(crate::service::page::Reply::data),
        Command::Api{query_file,variables_file,operation_name}=>{
            if query_file=="-" && variables_file.as_deref()==Some("-"){return Err(AppError::input("Only one input can read stdin"));}
            let query=read_body(&query_file)?;
            let variables=match variables_file{Some(p)=>serde_json::from_str(&read_body(&p)?).map_err(|_|AppError::input("Invalid variables JSON"))?,None=>json!({})};
            service.raw_api(query,variables,operation_name).await.map(crate::service::page::Reply::data)
        },
        Command::Issue{command:args::IssueCommand::View{reference}}=>service.view_issue(&reference).await.map(crate::service::page::Reply::data),
        Command::Issue{command:args::IssueCommand::List{filter,page}}=>service.list_issues(filter.into(),page.into()).await,
        Command::Team{command}=>catalog(&service,"team",command).await,
        Command::User{command}=>catalog(&service,"user",command).await,
        Command::State{command}=>catalog(&service,"state",command).await,
        Command::Label{command}=>catalog(&service,"label",command).await,
        _=>Err(AppError::new("api","Command not implemented yet")),
    }
}

impl From<args::Paging> for crate::service::page::PageRequest {fn from(p:args::Paging)->Self{Self{limit:p.limit.unwrap_or(50),after:p.after,all:p.all}}}
impl From<args::Filters> for crate::service::issues::IssueFilter {fn from(f:args::Filters)->Self{Self{team:f.team,assignee:f.assignee,project:f.project,state:f.state,search:f.search}}}
async fn catalog(service:&crate::service::Service,kind:&str,command:args::CatalogCommand)->crate::error::Result<crate::service::page::Reply>{
    let args::CatalogCommand::List{team,page}=command;
    if kind=="state" && team.is_none(){return Err(AppError::input("state list requires --team"));}
    if team.is_some() && !["state","label"].contains(&kind){return Err(AppError::input("--team only applies to states and labels"));}
    let mut filter=json!({});if let Some(team)=team{filter["team"]=json!({"id":{"eq":service.resolve("team",&team,None).await?}});}
    let (op,root)=crate::service::resolve::catalog(kind)?;service.list(op,root,filter,page.into()).await
}
