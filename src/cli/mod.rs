pub mod args;
mod dispatch;
pub mod render;
pub mod schema;
use crate::error::AppError;
use args::Cli;
use clap::{CommandFactory, Parser};
use dispatch::dispatch;
use serde_json::json;
use std::io::{IsTerminal, Write};

pub async fn run() -> i32 {
    let args: Vec<String> = std::env::args().collect();
    let json_mode = args.iter().any(|x| x == "--json" || x == "--output=json")
        || args
            .windows(2)
            .any(|x| x[0] == "--output" && x[1] == "json")
        || (!std::io::stdout().is_terminal()
            && !args.iter().any(|x| x == "--output=text")
            && !args
                .windows(2)
                .any(|x| x[0] == "--output" && x[1] == "text"));
    let cli = match Cli::try_parse_from(&args) {
        Ok(c) => c,
        Err(e)
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            return write_stdout(&e.to_string());
        }
        Err(e) => return render_error(AppError::input(e.to_string()), json_mode),
    };
    if let Some(args::Command::Api {
        query_file,
        query_path,
        ..
    }) = &cli.command
    {
        let message = match (query_file, query_path) {
            (None, None) => Some("Pass the file path or --query-file <path>"),
            (Some(a), Some(b)) if a != b => {
                Some("Conflicting query files: pass the file path or --query-file <path>")
            }
            _ => None,
        };
        if let Some(message) = message {
            return render_error(AppError::input(message), json_mode);
        }
    }
    if should_launch_tui(
        &cli,
        std::io::stdin().is_terminal(),
        std::io::stdout().is_terminal(),
    ) {
        return match crate::tui::run(cli.workspace.as_deref()).await {
            Ok(()) => 0,
            Err(e) => render_error(e, false),
        };
    }
    if cli.command.is_none() {
        return write_stdout(&Cli::command().render_help().to_string());
    }
    let explicit_json = cli.json || matches!(cli.output, Some(args::Output::Json));
    let view_fields = match &cli.command {
        Some(args::Command::Issue {
            command: args::IssueCommand::View { field, .. },
        }) => Some(field.clone()),
        _ => None,
    };
    let fields = view_fields.as_deref().filter(|f| !f.is_empty());
    // Field selection is for grep: plain stdout and plain stderr unless JSON is explicit.
    let json_mode = json_mode && (explicit_json || fields.is_none());
    if let Some(fields) = fields
        && let Err(e) = render::validate_fields(fields)
    {
        return render_error(e, json_mode);
    }
    match dispatch(cli).await {
        Ok(value) => {
            let text = if let Some(fields) = fields {
                if json_mode {
                    serde_json::to_string(&json!({"schema_version":1,
                        "data":render::select(&value.data, fields),"meta":value.meta}))
                    .expect("JSON values serialize")
                } else {
                    render::fields_text(&value.data, fields)
                }
            } else if view_fields.is_some() && !json_mode {
                render::issue_text(&value.data)
            } else if json_mode {
                serde_json::to_string(
                    &json!({"schema_version":1,"data":value.data,"meta":value.meta}),
                )
                .expect("JSON values serialize")
            } else {
                serde_json::to_string_pretty(&value.data).unwrap_or_default()
            };
            write_stdout(&text)
        }
        Err(e) => render_error(e, json_mode),
    }
}
fn render_error(error: AppError, json_mode: bool) -> i32 {
    let code = error.exit_code();
    if json_mode {
        let value = json!({"schema_version":1,"data":error.data,"error":error,"meta":error.meta});
        let _ = writeln!(std::io::stdout(), "{value}");
    } else {
        eprintln!(
            "{}: {}",
            render::printable(&error.code),
            render::printable(&error.message)
        );
    }
    code
}

fn write_stdout(text: &str) -> i32 {
    match writeln!(std::io::stdout().lock(), "{text}") {
        Ok(()) => 0,
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => 0,
        Err(_) => 9,
    }
}

/// Preserve noninteractive help and explicit machine output.
pub fn should_launch_tui(cli: &Cli, stdin_tty: bool, stdout_tty: bool) -> bool {
    cli.command.is_none()
        && stdin_tty
        && stdout_tty
        && !cli.json
        && !matches!(cli.output, Some(args::Output::Json))
}
