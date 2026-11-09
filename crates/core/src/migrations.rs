//! The guard on schema migration numbers: two branches that each append a migration both take
//! the next number. The schema file numbers its entries in comments (`// 16: what it does`); the
//! lines a branch added to that file name the numbers it took.

/// The file whose comments number the schema migrations.
pub const SCHEMA_FILE: &str = "crates/daemon/src/store/schema.rs";

/// A migration a branch added: its number and the first line of its description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub number: u32,
    pub name: String,
}

/// A number both the ship and the base added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    pub number: u32,
    /// The ship's migration with that number.
    pub ship: String,
    /// The base's migration with that number.
    pub base: String,
}

/// The migrations a diff of [`SCHEMA_FILE`] adds: its added lines of the form `// <n>: <text>`.
pub fn added(diff: &str) -> Vec<Migration> {
    diff.lines()
        .filter(|l| !l.starts_with("+++"))
        .filter_map(|l| l.strip_prefix('+'))
        .filter_map(|l| l.trim_start().strip_prefix("//"))
        .filter_map(|l| {
            let (number, name) = l.trim_start().split_once(':')?;
            Some(Migration {
                number: number.parse().ok()?,
                name: name.trim().to_owned(),
            })
        })
        .collect()
}

/// The numbers the ship and the base both added, in the ship's order.
pub fn clashes(ship: &[Migration], base: &[Migration]) -> Vec<Clash> {
    ship.iter()
        .filter_map(|s| {
            let b = base.iter().find(|b| b.number == s.number)?;
            Some(Clash {
                number: s.number,
                ship: s.name.clone(),
                base: b.name.clone(),
            })
        })
        .collect()
}

/// A clash as a worker reads it.
pub fn describe(c: &Clash, base_branch: &str) -> String {
    format!(
        "migration {} is taken twice: the ship adds \"{}\" and {base_branch} already has \"{}\"; renumber the ship's to the next free number",
        c.number, c.ship, c.base
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "--- a/x\n+++ b/x\n@@ -1 +1,3 @@\n+    // 16: the domains a project reads.\n+    r#\"\n+    // note: not a migration\n-    // 15: removed\n";

    #[test]
    fn reads_only_numbered_added_comments() {
        let found = added(DIFF);
        assert_eq!(
            found,
            vec![Migration {
                number: 16,
                name: "the domains a project reads.".to_owned()
            }]
        );
    }

    #[test]
    fn a_clash_needs_the_same_number_on_both_sides() {
        let m = |number, name: &str| Migration {
            number,
            name: name.to_owned(),
        };
        let ship = [m(16, "mine"), m(17, "more")];
        assert_eq!(
            clashes(&ship, &[m(16, "theirs")]),
            vec![Clash {
                number: 16,
                ship: "mine".to_owned(),
                base: "theirs".to_owned()
            }]
        );
        assert!(clashes(&ship, &[m(18, "theirs")]).is_empty());
    }
}
