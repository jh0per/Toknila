//! `tok chain rm`: delete a chain with its cards. The daemon decides what is allowed; this asks
//! the person to confirm first.

use std::io::{BufRead, IsTerminal};

use anyhow::{Result, bail};
use toknila_api_types::ChainDeletion;
use toknila_client::Client;

use super::emit;

/// Deletes `chain` (within `project` when given) after the person typed its name, or at once
/// with `--yes`.
pub(super) fn run(
    c: &Client,
    chain: &str,
    project: Option<&str>,
    yes: bool,
    json_out: bool,
) -> Result<()> {
    if !yes {
        let stdin = std::io::stdin();
        let terminal = stdin.is_terminal();
        confirm(&mut stdin.lock(), terminal, chain)?;
    }
    let out = c.delete_chain(chain, project)?;
    emit(json_out, &out, deleted);
    Ok(())
}

/// Asks for the chain's name. Refuses when nobody can answer (stdin is not a terminal) and when
/// the answer is not the name.
fn confirm(input: &mut impl BufRead, terminal: bool, name: &str) -> Result<()> {
    if !terminal {
        bail!(
            "not deleting chain {name}: it needs a confirmation and stdin is not a terminal; pass --yes"
        );
    }
    eprintln!("This deletes chain {name} with its cards, threads and ships' checkouts.");
    eprint!("Type the chain name ({name}) to go on: ");
    let mut line = String::new();
    input.read_line(&mut line)?;
    if line.trim() != name {
        bail!("{:?} is not {name}; nothing was deleted", line.trim());
    }
    Ok(())
}

/// What a deletion did, for a person.
fn deleted(d: &ChainDeletion) {
    println!(
        "chain {} of {} deleted: {} card(s)",
        d.chain.name, d.project, d.tickets
    );
    for w in &d.worktrees_removed {
        println!("removed worktree {w}");
    }
    for w in &d.warnings {
        println!("warning: {w}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_typed_name_confirms() {
        assert!(confirm(&mut "phase-1\n".as_bytes(), true, "phase-1").is_ok());
        let wrong = confirm(&mut "yes\n".as_bytes(), true, "phase-1").expect_err("wrong");
        assert!(wrong.to_string().contains("nothing was deleted"));
        let none = confirm(&mut "phase-1\n".as_bytes(), false, "phase-1").expect_err("no tty");
        assert!(none.to_string().contains("pass --yes"));
    }
}
