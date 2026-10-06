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
    render_error(AppError::new("api", "Command not implemented yet"), json_mode)
}
fn render_error(error: AppError, json_mode: bool) -> i32 {
    let code=error.exit_code();
    if json_mode {
        let value=json!({"schema_version":1,"data":error.data,"error":error,"meta":{"complete":false}});
        let _=writeln!(std::io::stdout(),"{value}");
    } else { eprintln!("{}: {}",error.code,error.message); }
    code
}
