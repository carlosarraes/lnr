use clap::{Args, Parser, Subcommand, ValueEnum};
#[derive(Debug, Parser)]
#[command(name="lnr", version, about="Linear workflows for agents and humans")]
pub struct Cli {
    #[arg(long, global=true, conflicts_with="output")]
    pub json: bool,
    #[arg(long, global=true, value_enum)]
    pub output: Option<Output>,
    #[arg(long, global=true)]
    pub workspace: Option<String>,
    #[command(subcommand)]
    pub command: Option<Command>,
}
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Output { Json, Text }
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect or import authentication
    Auth { #[command(subcommand)] command: AuthCommand },
    /// Read and change issues
    Issue { #[command(subcommand)] command: IssueCommand },
    /// Read project progress and issues
    Project { #[command(subcommand)] command: ProjectCommand },
    Team { #[command(subcommand)] command: CatalogCommand },
    User { #[command(subcommand)] command: CatalogCommand },
    State { #[command(subcommand)] command: CatalogCommand },
    Label { #[command(subcommand)] command: CatalogCommand },
    /// Execute GraphQL from a file; '-' reads stdin
    Api { #[arg(long)] query_file: String, #[arg(long)] variables_file: Option<String>, #[arg(long)] operation_name: Option<String> },
    /// Machine-readable command and response discovery
    Schema,
}
#[derive(Debug, Subcommand)]
pub enum AuthCommand { Status, ImportLinear { #[arg(long)] replace: bool } }
#[derive(Debug, Subcommand)]
pub enum CatalogCommand { List { #[arg(long)] team: Option<String>, #[command(flatten)] page: Paging } }
#[derive(Debug, Args, Clone)]
pub struct Paging {
    #[arg(long, value_parser=clap::value_parser!(u16).range(1..=250), conflicts_with="all")]
    pub limit: Option<u16>,
    #[arg(long)] pub after: Option<String>,
    #[arg(long)] pub all: bool,
}
impl Default for Paging { fn default() -> Self { Self {limit: Some(50), after: None, all: false} } }
#[derive(Debug, Args, Clone, Default)]
pub struct Filters {
    #[arg(long)] pub team: Option<String>,
    #[arg(long)] pub assignee: Option<String>,
    #[arg(long)] pub project: Option<String>,
    #[arg(long)] pub state: Option<String>,
    #[arg(long)] pub search: Option<String>,
}
#[derive(Debug, Subcommand)]
pub enum IssueCommand {
    List { #[command(flatten)] filter: Filters, #[command(flatten)] page: Paging },
    View { reference: String },
    Context { reference: String, #[arg(long,default_value="20",value_parser=clap::value_parser!(u16).range(1..=100))] limit: u16,
        #[arg(long)] comments_after: Option<String>, #[arg(long)] children_after: Option<String>,
        #[arg(long)] relations_after: Option<String>, #[arg(long)] inverse_relations_after: Option<String> },
    Create { #[arg(long)] team: String, #[arg(long)] title: String, #[command(flatten)] fields: WriteFields },
    Update { reference: String, #[arg(long)] title: Option<String>, #[command(flatten)] fields: WriteFields },
    Comment { #[command(subcommand)] command: CommentCommand },
    Relation { #[command(subcommand)] command: RelationCommand },
}
#[derive(Debug, Args, Default)]
pub struct WriteFields {
    #[arg(long)] pub state: Option<String>,
    #[arg(long,conflicts_with="unassign")] pub assignee: Option<String>,
    #[arg(long)] pub unassign: bool,
    #[arg(long,conflicts_with="clear_project")] pub project: Option<String>,
    #[arg(long)] pub clear_project: bool,
    #[arg(long,conflicts_with="clear_parent")] pub parent: Option<String>,
    #[arg(long)] pub clear_parent: bool,
    #[arg(long,conflicts_with="clear_labels",value_delimiter=',')] pub labels: Option<Vec<String>>,
    #[arg(long)] pub clear_labels: bool,
    #[arg(long,value_parser=clap::value_parser!(u8).range(0..=4))] pub priority: Option<u8>,
    #[arg(long,conflicts_with="clear_description")] pub description_file: Option<String>,
    #[arg(long)] pub clear_description: bool,
}
#[derive(Debug, Subcommand)]
pub enum CommentCommand {
    List { reference: String, #[command(flatten)] page: Paging },
    Add { reference: String, #[arg(long)] body_file: String },
}
#[derive(Debug, Subcommand)]
pub enum RelationCommand {
    List { reference: String, #[command(flatten)] page: Paging, #[arg(long)] inverse_after: Option<String> },
    Add { reference: String, #[arg(long,group="target")] blocks: Option<String>, #[arg(long,group="target")] related: Option<String>, #[arg(long,group="target")] duplicate_of: Option<String> },
    Remove { id: String },
}
#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    List { #[arg(long)] team: Option<String>, #[arg(long)] state: Option<String>, #[arg(long,conflicts_with="involvement")] mine: bool,
        #[arg(long,value_delimiter=',',value_parser=["lead","member","assignee"])] involvement: Vec<String>, #[command(flatten)] page: Paging },
    View { reference: String },
    Issues { reference: String, #[command(flatten)] filter: Filters, #[command(flatten)] page: Paging },
}
