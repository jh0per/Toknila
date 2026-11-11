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
    /// For a branch, the key of the merged or cancelled card it belonged to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<String>,
    /// For a branch, the commit on the base branch that merged it (the host looks it up).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_commit: Option<String>,
}

/// What the live cards of a project hold.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Owned {
    /// Branch name to the key of the merged or cancelled card that had it.
    pub retired: std::collections::BTreeMap<String, String>,
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
        .map(|mut s| {
            if s.kind == StrayKind::Branch {
                s.card = owned.retired.get(&s.name).cloned();
            }
            s
        })
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
            card: None,
            merge_commit: None,
        }
    }

    fn branch(name: &str, ahead: u32) -> Stray {
        Stray {
            kind: StrayKind::Branch,
            name: name.to_owned(),
            branch: None,
            ahead: Some(ahead),
            card: None,
            merge_commit: None,
        }
    }

    #[test]
    fn a_branch_of_a_merged_or_cancelled_card_is_listed_with_the_card_key() {
        let owned = Owned {
            retired: [("ship/c/done".to_owned(), "DEMO-4".to_owned())].into(),
            branches: ["ship/c/live".to_owned()].into(),
            ..Owned::default()
        };
        let found = [
            branch("ship/c/done", 0),
            branch("ship/c/live", 0),
            branch("ship/c/unknown", 1),
        ];
        let out = find(&found, &owned);
        let got: Vec<_> = out
            .iter()
            .map(|s| (s.name.as_str(), s.card.as_deref()))
            .collect();
        assert_eq!(
            got,
            [("ship/c/done", Some("DEMO-4")), ("ship/c/unknown", None)]
        );
    }

    #[test]
    fn what_a_live_card_holds_is_not_a_stray_and_the_rest_is() {
        let owned = Owned {
            worktrees: ["/s/a".to_owned()].into(),
            branches: ["ship/c/a".to_owned(), "ship/c/b".to_owned()].into(),
            ..Owned::default()
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
