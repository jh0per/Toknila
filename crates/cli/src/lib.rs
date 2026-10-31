//! Toknila CLI. Both `toknila` and `tok` call [`run`]; the binaries differ only in name.

use clap::Parser;

/// Toknila: plan work as chains of tickets and let a fleet of coding agents execute them.
#[derive(Debug, Parser)]
#[command(name = "toknila", version = toknila_core::VERSION, about, long_about = None)]
pub struct Cli {}

/// Parse arguments from the process and run the command.
pub fn run() {
    let _cli = Cli::parse();
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
