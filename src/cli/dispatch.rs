use super::args::{self, Cli};
use super::schema;
use crate::error::AppError;
use serde_json::json;
pub fn read_body(path: &str) -> crate::error::Result<String> {
    use std::io::Read;
    if path == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        Ok(s)
    } else {
        Ok(std::fs::read_to_string(path)?)
    }
}
pub async fn dispatch(cli: Cli) -> crate::error::Result<crate::service::page::Reply> {
    use args::{AuthCommand, Command};
    if let Some(Command::Auth {
        command: AuthCommand::Default { slug },
    }) = &cli.command
    {
        return crate::auth::set_default(slug)
            .await
            .map(crate::service::page::Reply::data);
    }
    if let Some(Command::Auth {
        command:
            AuthCommand::Login {
                client_id,
                no_browser,
                port,
                timeout,
                replace,
            },
    }) = &cli.command
    {
        return crate::auth::oauth::login(
            client_id,
            *no_browser,
            *port,
            *timeout,
            cli.workspace.as_deref(),
            *replace,
        )
        .await
        .map(crate::service::page::Reply::data);
    }
    if let Some(Command::Auth {
        command: AuthCommand::ImportLinear { replace },
    }) = &cli.command
    {
        let workspace = cli.workspace.as_deref().ok_or_else(|| {
            let mut e = crate::auth::missing_credentials();
            e.code = "invalid_input".into();
            e.message = format!("auth import-linear requires --workspace. {}", e.message);
            e
        })?;
        return crate::auth::import::import_linear(workspace, *replace)
            .await
            .map(crate::service::page::Reply::data);
    }
    if matches!(cli.command, Some(Command::Schema)) {
        return Ok(crate::service::page::Reply::data(schema::command_schema()));
    }
    if matches!(
        cli.command,
        Some(Command::Auth {
            command: AuthCommand::Status { check: false }
        })
    ) && cli.workspace.is_none()
    {
        return crate::auth::inventory().map(crate::service::page::Reply::data);
    }
    let service = crate::service::Service::new(crate::api::ApiClient::new(
        crate::auth::resolve(cli.workspace.as_deref()).await?,
    )?);
    let source = crate::auth::credential_source(cli.workspace.as_deref());
    let reference = cli.command.as_ref().and_then(issue_reference);
    match run(&service, cli.command.unwrap()).await {
        Err(e) if e.code == "not_found" && e.details.get("workspace").is_none() => {
            Err(explain_not_found(&service, e, &source, reference.as_deref()).await)
        }
        other => other,
    }
}

fn issue_reference(command: &args::Command) -> Option<String> {
    use args::{CommentCommand, IssueCommand, RelationCommand};
    let args::Command::Issue { command } = command else {
        return None;
    };
    match command {
        IssueCommand::View { reference, .. }
        | IssueCommand::Context { reference, .. }
        | IssueCommand::Update { reference, .. }
        | IssueCommand::Comments { reference, .. }
        | IssueCommand::Comment {
            command: CommentCommand::List { reference, .. } | CommentCommand::Add { reference, .. },
        }
        | IssueCommand::Relation {
            command:
                RelationCommand::List { reference, .. } | RelationCommand::Add { reference, .. },
        } => Some(reference.clone()),
        _ => None,
    }
}

/// Missing entities are often a credential for the wrong workspace, so name the
/// workspace and where the credential came from (never the credential itself).
async fn explain_not_found(
    service: &crate::service::Service,
    mut error: AppError,
    source: &str,
    reference: Option<&str>,
) -> AppError {
    let organization = service
        .status()
        .await
        .ok()
        .and_then(|s| s.organization)
        .map(|o| match o.name {
            Some(name) if name != o.url_key => format!("{} ({name})", o.url_key),
            _ => o.url_key,
        });
    let workspace = organization
        .clone()
        .unwrap_or_else(|| "unknown (organization lookup failed)".into());
    let target = reference.unwrap_or("the requested entity");
    let hint = format!(
        "this key is for workspace {workspace}; {target} isn't in it. Use the card text from your brief instead."
    );
    error.message = format!(
        "{} (credential: {source}; workspace: {workspace}). Hint: {hint}",
        error.message.trim_end_matches('.')
    );
    if !error.details.is_object() {
        error.details = Box::new(json!({}));
    }
    error.details["workspace"] = json!(organization);
    error.details["credential_source"] = json!(source);
    error.details["hint"] = json!(hint);
    error
}

async fn run(
    service: &crate::service::Service,
    command: args::Command,
) -> crate::error::Result<crate::service::page::Reply> {
    use args::{AuthCommand, Command};
    match command {
        Command::Auth {
            command: AuthCommand::Status { .. },
        } => service
            .status()
            .await
            .map(crate::service::page::Reply::data),
        Command::Api {
            query_file,
            query_path,
            variables_file,
            operation_name,
        } => {
            let query_file = query_file
                .or(query_path)
                .ok_or_else(|| AppError::input("Pass the file path or --query-file <path>"))?;
            if query_file == "-" && variables_file.as_deref() == Some("-") {
                return Err(AppError::input("Only one input can read stdin"));
            }
            let query = read_body(&query_file)?;
            let variables = match variables_file {
                Some(p) => serde_json::from_str(&read_body(&p)?)
                    .map_err(|_| AppError::input("Invalid variables JSON"))?,
                None => json!({}),
            };
            service
                .raw_api(query, variables, operation_name)
                .await
                .map(crate::service::page::Reply::data)
        }
        Command::Issue {
            command:
                args::IssueCommand::Context {
                    reference,
                    limit,
                    comments_after,
                    children_after,
                    relations_after,
                    inverse_relations_after,
                },
        } => service
            .issue_context(
                &reference,
                crate::service::context::ContextPageRequest {
                    limit,
                    comments_after,
                    children_after,
                    relations_after,
                    inverse_relations_after,
                },
            )
            .await
            .map(crate::service::page::Reply::into_json),
        Command::Issue {
            command:
                args::IssueCommand::View {
                    reference,
                    no_comments,
                    ..
                },
        } => service
            .inspect_issue(&reference, !no_comments)
            .await
            .map(crate::service::page::Reply::into_json),
        Command::Issue {
            command:
                args::IssueCommand::List { filter, page } | args::IssueCommand::Search { filter, page },
        } => service
            .list_issues(filter.into(), page.into())
            .await
            .map(crate::service::page::Reply::into_json),
        Command::Issue {
            command: args::IssueCommand::Ready { filter, page },
        } => service
            .ready_issues(filter.into(), page.into())
            .await
            .map(crate::service::page::Reply::into_json),
        Command::Issue {
            command:
                args::IssueCommand::Create {
                    team,
                    title,
                    fields,
                },
        } => service
            .create_issue(&team, title, write_fields(fields, None)?)
            .await
            .map(crate::service::page::Reply::data),
        Command::Issue {
            command:
                args::IssueCommand::Update {
                    reference,
                    title,
                    fields,
                },
        } => service
            .update_issue(&reference, write_fields(fields, title)?)
            .await
            .map(crate::service::page::Reply::data),
        Command::Issue {
            command: args::IssueCommand::Comments { reference, page },
        } => service
            .list_comments(&reference, page.into())
            .await
            .map(crate::service::page::Reply::into_json),
        Command::Issue {
            command: args::IssueCommand::Comment { command },
        } => match command {
            args::CommentCommand::List { reference, page } => service
                .list_comments(&reference, page.into())
                .await
                .map(crate::service::page::Reply::into_json),
            args::CommentCommand::Add { reference, content } => service
                .add_comment(&reference, comment_body(&content)?, content.mentions)
                .await
                .map(crate::service::page::Reply::data),
            args::CommentCommand::Edit { id, content } => service
                .edit_comment(&id, comment_body(&content)?, content.mentions)
                .await
                .map(crate::service::page::Reply::data),
            args::CommentCommand::Delete { id } => service
                .delete_comment(&id)
                .await
                .map(crate::service::page::Reply::data),
        },
        Command::Issue {
            command: args::IssueCommand::Relation { command },
        } => match command {
            args::RelationCommand::List {
                reference,
                page,
                inverse_after,
            } => service
                .list_relations(&reference, page.into(), inverse_after)
                .await
                .map(crate::service::page::Reply::into_json),
            args::RelationCommand::Remove { id } => service
                .remove_relation(&id)
                .await
                .map(crate::service::page::Reply::data),
            args::RelationCommand::Add {
                reference,
                blocks,
                related,
                duplicate_of,
            } => {
                let (kind, target) = if let Some(t) = blocks {
                    ("blocks", t)
                } else if let Some(t) = related {
                    ("related", t)
                } else if let Some(t) = duplicate_of {
                    ("duplicate", t)
                } else {
                    return Err(AppError::input(
                        "Specify --blocks, --related, or --duplicate-of",
                    ));
                };
                service
                    .add_relation(&reference, &target, kind)
                    .await
                    .map(crate::service::page::Reply::data)
            }
        },
        Command::Project { command } => match command {
            args::ProjectCommand::List {
                team,
                state,
                mine,
                involvement,
                page,
            } => service
                .list_projects(
                    crate::service::projects::ProjectFilter {
                        team,
                        state,
                        involvement: if mine {
                            vec!["lead".into(), "member".into(), "assignee".into()]
                        } else {
                            involvement
                        },
                    },
                    page.into(),
                )
                .await
                .map(crate::service::page::Reply::into_json),
            args::ProjectCommand::View { reference } => service
                .view_project(&reference)
                .await
                .map(crate::service::page::Reply::data),
            args::ProjectCommand::Issues {
                reference,
                filter,
                page,
            } => service
                .project_issues(&reference, filter.into(), page.into())
                .await
                .map(crate::service::page::Reply::into_json),
        },
        Command::Team { command } => catalog(service, "team", command).await,
        Command::User { command } => catalog(service, "user", command).await,
        Command::State {
            command: args::StateCommand::List { team, page },
        } => {
            catalog(
                service,
                "state",
                args::CatalogCommand::List {
                    team: Some(team),
                    page,
                },
            )
            .await
        }
        Command::Label { command } => catalog(service, "label", command).await,
        Command::Schema
        | Command::Auth {
            command:
                AuthCommand::ImportLinear { .. }
                | AuthCommand::Login { .. }
                | AuthCommand::Default { .. },
        } => unreachable!("handled before authentication"),
    }
}

impl From<args::Paging> for crate::service::page::PageRequest {
    fn from(p: args::Paging) -> Self {
        Self {
            limit: p.limit.unwrap_or(50),
            after: p.after,
            all: p.all,
        }
    }
}
impl From<args::Filters> for crate::service::issues::IssueFilter {
    fn from(f: args::Filters) -> Self {
        Self {
            team: f.team,
            assignee: f.assignee,
            project: f.project,
            state: f.state,
            search: f.search,
            title: f.title,
        }
    }
}
async fn catalog(
    service: &crate::service::Service,
    kind: &str,
    command: args::CatalogCommand,
) -> crate::error::Result<crate::service::page::Reply> {
    let args::CatalogCommand::List { team, page } = command;
    if kind == "state" && team.is_none() {
        return Err(AppError::input("state list requires --team"));
    }
    if team.is_some() && !["state", "label"].contains(&kind) {
        return Err(AppError::input("--team only applies to states and labels"));
    }
    let mut filter = json!({});
    if let Some(team) = team {
        let id = service.resolve("team", &team, None).await?;
        if kind == "label" {
            filter = crate::service::resolve::label_scope(&id);
        } else {
            filter["team"] = json!({"id":{"eq":id}});
        }
    }
    let (op, root) = crate::service::resolve::catalog(kind)?;
    service.list(op, root, filter, page.into()).await
}

fn write_fields(
    f: args::WriteFields,
    title: Option<String>,
) -> crate::error::Result<crate::service::writes::IssuePatch> {
    use crate::service::writes::{IssuePatch, Patch};
    fn patch<T>(value: Option<T>, clear: bool) -> Patch<T> {
        if clear {
            Patch::Clear
        } else if let Some(v) = value {
            Patch::Set(v)
        } else {
            Patch::Keep
        }
    }
    Ok(IssuePatch {
        title,
        state: f.state,
        assignee: patch(f.assignee, f.unassign),
        project: patch(f.project, f.clear_project),
        parent: patch(f.parent, f.clear_parent),
        labels: patch(f.labels, f.clear_labels),
        priority: f.priority,
        description: patch(
            f.description
                .or(f.description_file.map(|p| read_body(&p)).transpose()?),
            f.clear_description,
        ),
    })
}

fn comment_body(content: &args::CommentContent) -> crate::error::Result<String> {
    if let Some(body) = &content.body {
        Ok(body.clone())
    } else if let Some(path) = &content.body_file {
        read_body(path)
    } else {
        Err(AppError::input("Specify --body or --body-file"))
    }
}
