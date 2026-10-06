pub mod args;
mod dispatch;
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
    if cli.command.is_none() {
        return write_stdout(&Cli::command().render_help().to_string());
    }
    match dispatch(cli).await {
        Ok(value) => {
            let text = if json_mode {
                json!({"schema_version":1,"data":value.data,"meta":value.meta}).to_string()
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
        let value =
            json!({"schema_version":1,"data":error.data,"error":error,"meta":{"complete":false}});
        let _ = writeln!(std::io::stdout(), "{value}");
    } else {
        eprintln!("{}: {}", error.code, error.message);
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
