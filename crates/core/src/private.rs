//! The person's private folders, which no agent run may read: the one list, and the Claude Code
//! deny rules that keep an agent out of them. Pure: the daemon supplies the home folder and the
//! data-home entries and writes the rules into every run's `settings.json`.

/// Paths below the person's home folder (forward slashes) that hold what is theirs alone: the
/// Claude Code home and its config file (every conversation of the owner), keys, keychains and
/// the shell history files.
pub const PRIVATE_HOME_PATHS: [&str; 12] = [
    ".claude",
    ".claude.json",
    ".ssh",
    ".gnupg",
    "Library/Keychains",
    ".bash_history",
    ".zsh_history",
    ".zsh_sessions",
    ".sh_history",
    ".history",
    ".local/share/fish/fish_history",
    ".local/share/fish/fish_history.bak",
];

/// The tools whose file access a deny rule names a path for. Claude Code takes a path rule for
/// `Read` only, and applies it to every file-reading tool (`Grep` and `Glob` too); a `Grep(path)`
/// or `Glob(path)` rule is refused with a warning. `Bash` has its own command patterns.
pub const GUARDED_TOOLS: [&str; 2] = ["Read", "Bash"];

/// A path in the form of Claude Code's rule syntax (`//` + the path from the filesystem root;
/// a Windows drive `C:\x` reads `/c/x`).
fn rooted(abs: &str) -> String {
    let p = abs.replace('\\', "/");
    let mut chars = p.chars();
    match (chars.next(), chars.next()) {
        (Some(d), Some(':')) if d.is_ascii_alphabetic() => {
            format!("/{}{}", d.to_ascii_lowercase(), &p[2..])
        }
        _ => p,
    }
}

/// The deny rules for one absolute path (a file or a folder): `Read` (so also `Grep` and `Glob`)
/// on the path and everything below it, and `Bash` on any command that names it, spelled
/// absolutely, as `~/` or as `$HOME/` when it lies below `home`.
pub fn deny_rules_for(abs: &str, home: &str) -> Vec<String> {
    let path = rooted(abs.trim_end_matches(['/', '\\']));
    let mut out = vec![format!("Read(/{path})"), format!("Read(/{path}/**)")];
    out.push(format!("Bash(*{path}*)"));
    let home = rooted(home.trim_end_matches(['/', '\\']));
    if let Some(rel) = path.strip_prefix(&format!("{home}/")) {
        out.push(format!("Bash(*~/{rel}*)"));
        out.push(format!("Bash(*$HOME/{rel}*)"));
    }
    out
}

/// The allow rule that lets an agent `Read` exactly one absolute file (nothing below or beside it).
pub fn read_allow_rule(abs: &str) -> String {
    format!("Read(/{})", rooted(abs))
}

/// The absolute private paths below `home`.
pub fn private_paths(home: &str) -> Vec<String> {
    let home = home.trim_end_matches(['/', '\\']);
    PRIVATE_HOME_PATHS
        .iter()
        .map(|rel| format!("{home}/{rel}"))
        .collect()
}

/// The deny rules for the private home paths plus `extra` absolute paths (the Toknila data home's
/// entries other than the run's own folder).
pub fn deny_rules(home: &str, extra: &[String]) -> Vec<String> {
    private_paths(home)
        .iter()
        .chain(extra)
        .flat_map(|p| deny_rules_for(p, home))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_list_is_the_documented_one() {
        assert_eq!(
            PRIVATE_HOME_PATHS,
            [
                ".claude",
                ".claude.json",
                ".ssh",
                ".gnupg",
                "Library/Keychains",
                ".bash_history",
                ".zsh_history",
                ".zsh_sessions",
                ".sh_history",
                ".history",
                ".local/share/fish/fish_history",
                ".local/share/fish/fish_history.bak",
            ]
        );
    }

    #[test]
    fn a_path_below_home_is_denied_for_every_tool_in_every_spelling() {
        let rules = deny_rules_for("/Users/p/.ssh", "/Users/p");
        assert_eq!(
            rules,
            [
                "Read(//Users/p/.ssh)",
                "Read(//Users/p/.ssh/**)",
                "Bash(*/Users/p/.ssh*)",
                "Bash(*~/.ssh*)",
                "Bash(*$HOME/.ssh*)",
            ]
        );
    }

    #[test]
    fn a_path_outside_home_gets_no_home_spellings_and_a_drive_is_rooted() {
        let rules = deny_rules_for("/data/toknila/db", "/Users/p");
        assert_eq!(rules.len(), 3);
        assert!(!rules.iter().any(|r| r.contains('~')));
        assert_eq!(
            deny_rules_for(r"C:\Users\p\.ssh", r"C:\Users\p")[0],
            "Read(//c/Users/p/.ssh)"
        );
    }

    #[test]
    fn every_listed_folder_is_denied_for_all_four_tools() {
        let rules = deny_rules("/Users/p", &["/Users/p/.toknila/tokens".to_owned()]);
        for rel in PRIVATE_HOME_PATHS.iter().chain(&[".toknila/tokens"]) {
            for tool in GUARDED_TOOLS {
                let want = format!("{tool}(");
                assert!(
                    rules
                        .iter()
                        .any(|r| r.starts_with(&want) && r.contains(&format!("/Users/p/{rel}"))),
                    "{tool} {rel}"
                );
            }
        }
    }
}
