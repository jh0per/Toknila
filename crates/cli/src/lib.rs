//! Toknila CLI. Both `toknila` and `tok` call [`run`]; the binaries differ only in name.
//! The CLI is a thin client of the daemon API (it starts the daemon when needed);
//! `toknila daemon run` is the daemon itself. Business rules live in core, not here.

mod allow_always;
mod check_words;
mod client;
mod cmd;
mod daemon_cmd;
mod output;
mod paused_output;
mod review_output;
#[cfg(windows)]
mod win;

use std::process::ExitCode;

use clap::Parser;

pub use cmd::Command;

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

/// Parse arguments from the process and run the command.
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        use clap::CommandFactory;
        let _ = Cli::command().print_help();
        return ExitCode::SUCCESS;
    };
    match cmd::run(command, cli.json) {
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
