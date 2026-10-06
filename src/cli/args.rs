use clap::{Args, Parser, Subcommand, ValueEnum};
#[derive(Debug, Parser)]
#[command(
    name = "lnr",
    version,
    about = "Linear workflows for agents and humans"
)]
pub struct Cli {
    #[arg(long, global = true, conflicts_with = "output")]
    /// Emit the versioned JSON envelope
    pub json: bool,
    #[arg(long, global = true, value_enum)]
    /// Output format; defaults to JSON when piped
    pub output: Option<Output>,
    #[arg(long, global = true)]
    /// Stored workspace slug; see auth status for configured and importable slugs
    pub workspace: Option<String>,
    #[command(subcommand)]
    pub command: Option<Command>,
}
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Output {
    Json,
    Text,
}
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect or import authentication
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Read and change issues
    Issue {
        #[command(subcommand)]
        command: IssueCommand,
    },
    /// Read project progress and issues
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Discover workspace teams and their keys
    Team {
        #[command(subcommand)]
        command: CatalogCommand,
    },
    /// Discover workspace users and assignees
    User {
        #[command(subcommand)]
        command: CatalogCommand,
    },
    /// Discover team workflow states and their IDs
    State {
        #[command(subcommand)]
        command: StateCommand,
    },
    /// Discover workspace and team issue labels
    Label {
        #[command(subcommand)]
        command: CatalogCommand,
    },
    /// Execute GraphQL from a file; '-' reads stdin
    Api {
        #[arg(long)]
        /// GraphQL document file; '-' reads stdin
        query_file: String,
        #[arg(long)]
        /// JSON variables object file; '-' reads stdin
        variables_file: Option<String>,
        #[arg(long)]
        /// Operation to execute when a document contains multiple operations
        operation_name: Option<String>,
    },
    /// Machine-readable command and response discovery
    Schema,
}
#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Choose the stored workspace used when --workspace is omitted
    Default {
        /// Configured workspace slug, listed by auth status
        slug: String,
    },
    /// Sign in through your browser using OAuth with PKCE; store tokens in the configured credential store
    Login {
        /// OAuth application client ID; defaults to the original lnr application's ID
        #[arg(
            long,
            env = "LNR_CLIENT_ID",
            default_value = "85d20567e5636f5dccc9a369bb6a16dc"
        )]
        client_id: String,
        /// Print the authorization URL without opening a browser; useful with SSH port forwarding
        #[arg(long)]
        no_browser: bool,
        /// Loopback callback port; register http://127.0.0.1:PORT/callback in your OAuth app
        #[arg(long, default_value = "8484", value_parser=clap::value_parser!(u16).range(1..))]
        port: u16,
        /// Seconds to wait for browser authorization
        #[arg(long, default_value = "180", value_parser=clap::value_parser!(u16).range(1..=600))]
        timeout: u16,
        /// Replace an existing workspace credential
        #[arg(long)]
        replace: bool,
    },
    /// List configured/importable workspaces; --workspace verifies one with Linear
    Status {
        /// Verify the active credential with Linear instead of listing local workspaces
        #[arg(long)]
        check: bool,
    },
    /// Import an existing Linear CLI credential into the configured credential store
    ImportLinear {
        #[arg(long)]
        /// Replace an existing workspace credential
        replace: bool,
    },
}
#[derive(Debug, Subcommand)]
pub enum CatalogCommand {
    /// List matching items with cursor pagination
    List {
        #[arg(long)]
        /// Team key, exact name, or UUID
        team: Option<String>,
        #[command(flatten)]
        page: Paging,
    },
}
#[derive(Debug, Args, Clone)]
pub struct Paging {
    #[arg(long, value_parser=clap::value_parser!(u16).range(1..=250), conflicts_with="all")]
    /// Maximum items per page, 1-250; default 50
    pub limit: Option<u16>,
    #[arg(long)]
    /// Continue from meta.page.end_cursor
    pub after: Option<String>,
    #[arg(long)]
    /// Fetch every page; conflicts with --limit
    pub all: bool,
}
impl Default for Paging {
    fn default() -> Self {
        Self {
            limit: Some(50),
            after: None,
            all: false,
        }
    }
}
#[derive(Debug, Args, Clone, Default)]
pub struct Filters {
    #[arg(long)]
    /// Team key, exact name, or UUID
    pub team: Option<String>,
    #[arg(long)]
    /// Assignee UUID, exact name, display name, email, or me
    pub assignee: Option<String>,
    #[arg(long)]
    /// Project UUID or exact name
    pub project: Option<String>,
    #[arg(long)]
    /// State UUID, exact name (requires --team), or type: triage, backlog, unstarted, started, completed, canceled
    pub state: Option<String>,
    #[arg(long)]
    /// Search issue content for this text
    pub search: Option<String>,
}
#[derive(Debug, Subcommand)]
pub enum IssueCommand {
    /// List matching items with cursor pagination
    List {
        #[command(flatten)]
        filter: Filters,
        #[command(flatten)]
        page: Paging,
    },
    /// Read an issue and all comments, oldest first; --no-comments skips discussion
    View {
        /// Issue identifier such as ENG-123, or UUID
        reference: String,
        /// Omit comments; default view fetches all pages, oldest first
        #[arg(long)]
        no_comments: bool,
    },
    /// Read an issue with project, parent, children, comments, and relations
    Context {
        /// Issue identifier such as ENG-123, or UUID
        reference: String,
        #[arg(long,default_value="20",value_parser=clap::value_parser!(u16).range(1..=100))]
        /// Maximum items in each context collection, 1-100; default 20
        limit: u16,
        #[arg(long)]
        /// Continue comments from their collection cursor
        comments_after: Option<String>,
        #[arg(long)]
        /// Continue children from their collection cursor
        children_after: Option<String>,
        #[arg(long)]
        /// Continue outgoing relations from their collection cursor
        relations_after: Option<String>,
        #[arg(long)]
        /// Continue incoming relations from their collection cursor
        inverse_relations_after: Option<String>,
    },
    /// Create an issue in a team
    Create {
        #[arg(long)]
        /// Team key, exact name, or UUID
        team: String,
        #[arg(long)]
        /// Issue title
        title: String,
        #[command(flatten)]
        fields: WriteFields,
    },
    /// Update specified fields of an issue; omitted fields stay unchanged
    Update {
        /// Issue identifier such as ENG-123, or UUID
        reference: String,
        #[arg(long)]
        /// Issue title
        title: Option<String>,
        #[command(flatten)]
        fields: WriteFields,
    },
    /// Read or add issue comments
    Comment {
        #[command(subcommand)]
        command: CommentCommand,
    },
    /// Read, add, or remove issue relations
    Relation {
        #[command(subcommand)]
        command: RelationCommand,
    },
}
#[derive(Debug, Args, Default)]
pub struct WriteFields {
    #[arg(long)]
    /// Workflow state UUID or exact name within the issue team
    pub state: Option<String>,
    #[arg(long, conflicts_with = "unassign")]
    /// Assignee UUID, exact name, display name, email, or me
    pub assignee: Option<String>,
    #[arg(long)]
    /// Remove the assignee
    pub unassign: bool,
    #[arg(long, conflicts_with = "clear_project")]
    /// Project UUID or exact name
    pub project: Option<String>,
    #[arg(long)]
    /// Remove the project
    pub clear_project: bool,
    #[arg(long, conflicts_with = "clear_parent")]
    /// Parent issue identifier or UUID
    pub parent: Option<String>,
    #[arg(long)]
    /// Remove the parent issue
    pub clear_parent: bool,
    #[arg(long, conflicts_with = "clear_labels", value_delimiter = ',')]
    /// Comma-separated label UUIDs or exact names; replaces existing labels
    pub labels: Option<Vec<String>>,
    #[arg(long)]
    /// Remove all labels
    pub clear_labels: bool,
    #[arg(long,value_parser=clap::value_parser!(u8).range(0..=4))]
    /// Priority: 0 = none, 1 = urgent, 2 = high, 3 = normal, 4 = low
    pub priority: Option<u8>,
    #[arg(long, conflicts_with = "clear_description")]
    /// Read Markdown description from file; '-' reads stdin
    pub description_file: Option<String>,
    #[arg(long)]
    /// Remove the description
    pub clear_description: bool,
}
#[derive(Debug, Subcommand)]
pub enum CommentCommand {
    /// List matching items with cursor pagination
    List {
        /// Issue identifier such as ENG-123, or UUID
        reference: String,
        #[command(flatten)]
        page: Paging,
    },
    /// Add an item to an issue
    Add {
        /// Issue identifier such as ENG-123, or UUID
        reference: String,
        #[arg(long)]
        /// Read Markdown comment from file; '-' reads stdin
        body_file: String,
    },
}
#[derive(Debug, Subcommand)]
pub enum RelationCommand {
    /// List matching items with cursor pagination
    List {
        /// Issue identifier such as ENG-123, or UUID
        reference: String,
        #[command(flatten)]
        page: Paging,
        #[arg(long)]
        /// Cursor for incoming relations
        inverse_after: Option<String>,
    },
    #[command(group(clap::ArgGroup::new("target").required(true).args(["blocks","related","duplicate_of"])))]
    /// Add an item to an issue
    Add {
        /// Issue identifier such as ENG-123, or UUID
        reference: String,
        #[arg(long, group = "target")]
        /// Target issue identifier or UUID that this issue blocks
        blocks: Option<String>,
        #[arg(long, group = "target")]
        /// Related issue identifier or UUID
        related: Option<String>,
        #[arg(long, group = "target")]
        /// Original issue identifier or UUID that this issue duplicates
        duplicate_of: Option<String>,
    },
    /// Delete a relation by its UUID
    Remove {
        /// Relation UUID to delete, obtained from relation list
        id: String,
    },
}
#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    /// List matching items with cursor pagination
    List {
        #[arg(long)]
        /// Team key, exact name, or UUID
        team: Option<String>,
        #[arg(long)]
        /// Project state, for example planned, started, paused, completed, canceled
        state: Option<String>,
        #[arg(long, conflicts_with = "involvement")]
        /// Projects where you are lead, member, or assigned an issue
        mine: bool,
        #[arg(long,value_delimiter=',',value_parser=["lead","member","assignee"])]
        /// Match any listed role for the current user
        involvement: Vec<String>,
        #[command(flatten)]
        page: Paging,
    },
    /// Read one item by reference
    View {
        /// Project UUID or exact name
        reference: String,
    },
    /// List issues belonging to a project
    Issues {
        /// Project UUID or exact name
        reference: String,
        #[command(flatten)]
        filter: Filters,
        #[command(flatten)]
        page: Paging,
    },
}

#[derive(Debug, Subcommand)]
pub enum StateCommand {
    /// List matching items with cursor pagination
    List {
        #[arg(long)]
        /// Team key, exact name, or UUID
        team: String,
        #[command(flatten)]
        page: Paging,
    },
}
