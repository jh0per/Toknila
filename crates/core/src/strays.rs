//! Ships and branches no live card owns: what a cancelled or deleted card left in the repository.
//! Pure: the host says what it found and which names the live cards hold; the rule is here.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// What kind of leftover.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrayKind {
    /// A ship's worktree.
    Worktree,
    /// A `ship/*` branch.
    Branch,
}

/// One worktree or branch the host found under Toknila's ships.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stray {
    pub kind: StrayKind,
    /// The worktree's path, or the branch's name.
    pub name: String,
    /// For a worktree, the branch checked out in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// For a branch, its commits that the project's base branch does not have; deleting it
    /// loses them. `None` when the host could not count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ahead: Option<u32>,
}

/// What the live cards of a project hold.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Owned {
    /// Worktree paths, in the host's canonical spelling.
    pub worktrees: BTreeSet<String>,
    /// Branch names.
    pub branches: BTreeSet<String>,
}

/// The ones of `found` that no live card owns. A worktree is owned by its path or by the branch
/// checked out in it (a ship being made has its branch before the card records its path).
pub fn find(found: &[Stray], owned: &Owned) -> Vec<Stray> {
    found
        .iter()
        .filter(|s| match s.kind {
            StrayKind::Worktree => {
                !owned.worktrees.contains(&s.name)
                    && !s
                        .branch
                        .as_ref()
                        .is_some_and(|b| owned.branches.contains(b))
            }
            StrayKind::Branch => !owned.branches.contains(&s.name),
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(path: &str, branch: Option<&str>) -> Stray {
        Stray {
            kind: StrayKind::Worktree,
            name: path.to_owned(),
            branch: branch.map(str::to_owned),
            ahead: None,
        }
    }

    fn branch(name: &str, ahead: u32) -> Stray {
        Stray {
            kind: StrayKind::Branch,
            name: name.to_owned(),
            branch: None,
            ahead: Some(ahead),
        }
    }

    #[test]
    fn what_a_live_card_holds_is_not_a_stray_and_the_rest_is() {
        let owned = Owned {
            worktrees: ["/s/a".to_owned()].into(),
            branches: ["ship/c/a".to_owned(), "ship/c/b".to_owned()].into(),
        };
        let found = [
            tree("/s/a", Some("ship/c/a")),
            tree("/s/gone", Some("ship/c/gone")),
            tree("/s/b-new", Some("ship/c/b")),
            tree("/s/detached", None),
            branch("ship/c/a", 0),
            branch("ship/c/gone", 2),
        ];
        let names: Vec<_> = find(&found, &owned).into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["/s/gone", "/s/detached", "ship/c/gone"]);
    }

    #[test]
    fn nothing_owned_makes_everything_a_stray_and_nothing_found_makes_none() {
        let found = [tree("/s/a", None), branch("ship/x", 0)];
        assert_eq!(find(&found, &Owned::default()).len(), 2);
        assert!(find(&[], &Owned::default()).is_empty());
    }
}
