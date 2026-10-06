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
        command: AuthCommand::ImportLinear { replace },
    }) = &cli.command
    {
        let workspace = cli
            .workspace
            .as_deref()
            .ok_or_else(|| AppError::input("auth import-linear requires --workspace"))?;
        return crate::auth::import::import_linear(workspace, *replace)
            .await
            .map(crate::service::page::Reply::data);
    }
    if matches!(cli.command, Some(Command::Schema)) {
        return Ok(crate::service::page::Reply::data(schema::command_schema()));
    }
    let service = crate::service::Service::new(crate::api::ApiClient::new(crate::auth::resolve(
        cli.workspace.as_deref(),
    )?)?);
    match cli.command.unwrap() {
        Command::Auth {
            command: AuthCommand::Status,
        } => service
            .status()
            .await
            .map(crate::service::page::Reply::data),
        Command::Api {
            query_file,
            variables_file,
            operation_name,
        } => {
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
            command: args::IssueCommand::View { reference },
        } => service
            .view_issue(&reference)
            .await
            .map(crate::service::page::Reply::data),
        Command::Issue {
            command: args::IssueCommand::List { filter, page },
        } => service
            .list_issues(filter.into(), page.into())
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
            command: args::IssueCommand::Comment { command },
        } => match command {
            args::CommentCommand::List { reference, page } => service
                .list_comments(&reference, page.into())
                .await
                .map(crate::service::page::Reply::into_json),
            args::CommentCommand::Add {
                reference,
                body_file,
            } => service
                .add_comment(&reference, read_body(&body_file)?)
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
        Command::Team { command } => catalog(&service, "team", command).await,
        Command::User { command } => catalog(&service, "user", command).await,
        Command::State {
            command: args::StateCommand::List { team, page },
        } => {
            catalog(
                &service,
                "state",
                args::CatalogCommand::List {
                    team: Some(team),
                    page,
                },
            )
            .await
        }
        Command::Label { command } => catalog(&service, "label", command).await,
        Command::Schema
        | Command::Auth {
            command: AuthCommand::ImportLinear { .. },
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
            f.description_file.map(|p| read_body(&p)).transpose()?,
            f.clear_description,
        ),
    })
}
