use clap::Parser;
use lnr::cli::{args::Cli, should_launch_tui};
#[test]
fn only_interactive_non_json_invocations_launch() {
    for args in [
        vec!["lnr"],
        vec!["lnr", "--workspace", "demo"],
        vec!["lnr", "--output=text"],
    ] {
        let cli = Cli::parse_from(args);
        assert!(should_launch_tui(&cli, true, true));
        assert!(!should_launch_tui(&cli, false, true));
        assert!(!should_launch_tui(&cli, true, false));
    }
    for args in [
        vec!["lnr", "--json"],
        vec!["lnr", "--output=json"],
        vec!["lnr", "project", "list"],
    ] {
        assert!(!should_launch_tui(&Cli::parse_from(args), true, true));
    }
}
#[test]
fn redirected_empty_invocation_still_shows_help() {
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_lnr"))
        .output()
        .unwrap();
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).contains("Usage:"));
    assert!(!o.stdout.contains(&27));
}
