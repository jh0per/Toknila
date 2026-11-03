//! Toknila CLI. Both `toknila` and `tok` call [`run`]; the binaries differ only in name.
//! The CLI is a thin client of the daemon API; `toknila daemon run` is the daemon itself.

mod daemon_cmd;
#[cfg(windows)]
mod win;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Toknila: plan work as chains of tickets and let a fleet of coding agents execute them.
#[derive(Debug, Parser)]
#[command(name = "toknila", version = toknila_core::VERSION, about, long_about = None)]
pub struct Cli {
    /// Print machine-readable JSON instead of a table.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start, stop or inspect the background daemon.
    #[command(subcommand)]
    Daemon(daemon_cmd::DaemonCmd),
}

/// Parse arguments from the process and run the command.
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        None => {
            use clap::CommandFactory;
            let _ = Cli::command().print_help();
            Ok(())
        }
        Some(Command::Daemon(cmd)) => daemon_cmd::run(cmd, cli.json),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_consistent() {
        Cli::command().debug_assert();
    }
}
