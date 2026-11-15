//! A project's extra checks: commands that run after the check command when the ship changed
//! files that match their path globs (`apps/web/**` runs the web gates only for web changes).

use serde::{Deserialize, Serialize};

/// One extra check: `command` runs when a changed file matches one of `paths`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtraCheck {
    /// Globs relative to the repository root: `*` is any run of characters inside a folder name,
    /// `**` any run including `/`, `?` one character.
    pub paths: Vec<String>,
    /// The command, one line like the check command.
    pub command: String,
}

impl ExtraCheck {
    /// The check from the CLI's `glob[,glob]=command` form.
    pub fn parse(text: &str) -> Result<Self, String> {
        let (paths, command) = text.split_once('=').ok_or(
            "an extra check is written paths=command, such as 'apps/web/**=cargo xtask web'",
        )?;
        Self {
            paths: paths.split(',').map(str::to_owned).collect(),
            command: command.to_owned(),
        }
        .clean()
    }

    /// The check as a person typed it: paths and command trimmed; both must be there, and the
    /// command is one line without parentheses (it goes into an agent's permission rule).
    pub fn clean(self) -> Result<Self, String> {
        let paths: Vec<String> = self
            .paths
            .iter()
            .map(|p| p.trim().to_owned())
            .filter(|p| !p.is_empty())
            .collect();
        if paths.is_empty() || paths.iter().any(|p| p.contains(['\n', '\r'])) {
            return Err("an extra check names the paths it is for, such as apps/web/**".to_owned());
        }
        let command = crate::Project::clean_check_command(&self.command)
            .map_err(|e| e.replace("check command", "extra check command"))?
            .ok_or("an extra check needs a command")?;
        Ok(Self { paths, command })
    }

    /// The list as a person typed it, every entry cleaned.
    pub fn clean_all(list: Vec<Self>) -> Result<Vec<Self>, String> {
        list.into_iter().map(Self::clean).collect()
    }

    /// The paths as the round and the settings show them: `apps/web/**`.
    pub fn label(&self) -> String {
        self.paths.join(", ")
    }

    /// Whether one of `files` (paths with `/`) matches one of the globs.
    pub fn matches(&self, files: &[String]) -> bool {
        let globs: Vec<Vec<char>> = self.paths.iter().map(|p| p.chars().collect()).collect();
        files.iter().any(|f| {
            let name: Vec<char> = f.chars().collect();
            globs.iter().any(|g| glob(g, &name))
        })
    }
}

/// `pattern` against `name`: `**` any run, `*` any run without `/`, `?` one character but `/`.
fn glob(pattern: &[char], name: &[char]) -> bool {
    match pattern {
        [] => name.is_empty(),
        ['*', '*', rest @ ..] => {
            (0..=name.len()).any(|i| glob(rest, &name[i..]))
                // `**/x` also matches `x` itself.
                || rest.first() == Some(&'/') && glob(&rest[1..], name)
        }
        ['*', rest @ ..] => {
            let room = name.iter().position(|c| *c == '/').unwrap_or(name.len());
            (0..=room).any(|i| glob(rest, &name[i..]))
        }
        ['?', rest @ ..] => name.first().is_some_and(|c| *c != '/') && glob(rest, &name[1..]),
        [c, rest @ ..] => name.first() == Some(c) && glob(rest, &name[1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(paths: &[&str]) -> ExtraCheck {
        ExtraCheck {
            paths: paths.iter().map(|p| (*p).to_owned()).collect(),
            command: "x".to_owned(),
        }
    }

    fn files(f: &[&str]) -> Vec<String> {
        f.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn globs_match_the_way_people_write_them() {
        let web = check(&["apps/web/**"]);
        assert!(web.matches(&files(&["crates/a.rs", "apps/web/src/a/b.ts"])));
        assert!(!web.matches(&files(&["crates/a.rs", "apps/webby/x.ts"])));
        assert!(check(&["*.md"]).matches(&files(&["README.md"])));
        assert!(!check(&["*.md"]).matches(&files(&["docs/a.md"])));
        assert!(check(&["**/*.md"]).matches(&files(&["docs/a.md", "b.md"])));
        assert!(check(&["a/?.rs"]).matches(&files(&["a/b.rs"])));
        assert!(check(&["x/**", "y/**"]).matches(&files(&["y/z"])));
        assert!(!web.matches(&[]));
    }

    #[test]
    fn the_cli_form_is_paths_equals_command() {
        let c = ExtraCheck::parse("apps/web/**=cargo xtask web && cargo xtask e2e-web")
            .expect("parses");
        assert_eq!(c.paths, ["apps/web/**"]);
        assert_eq!(c.command, "cargo xtask web && cargo xtask e2e-web");
        let two = ExtraCheck::parse("a/**, b/** = make").expect("parses");
        assert_eq!(two.paths, ["a/**", "b/**"]);
        assert!(ExtraCheck::parse("no equals sign").is_err());
        assert!(ExtraCheck::parse("=make").is_err());
        assert!(ExtraCheck::parse("a/**=").is_err());
        assert!(ExtraCheck::parse("a/**=run (x)").is_err());
    }
}
