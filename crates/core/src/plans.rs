//! Plans a repository holds: spec folders (`specs/<name>/tasks.md`) and chain files
//! (`plans/<name>.toml`). The host lists those two folders one level deep and reads the plan
//! files; this module says which entries are plans, counts their cards and tells whether a chain
//! of the project was imported from one. Nothing here walks a tree or reads a file.

use serde::{Deserialize, Serialize};

use crate::plan::Plan;

/// The folder of spec folders, relative to the repository.
pub const SPECS_DIR: &str = "specs";
/// The folder of chain files, relative to the repository.
pub const PLANS_DIR: &str = "plans";
/// The file that makes a folder a spec folder.
pub const TASKS_FILE: &str = "tasks.md";
/// The title of a spec folder's chain is the first heading of this file.
pub const SPEC_FILE: &str = "spec.md";
/// The folder a spec toolkit keeps its own files in; it says the repository uses one.
pub const SPECIFY_DIR: &str = ".specify";

/// One of the two places a repository keeps plans in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanDir {
    Specs,
    Plans,
}

impl PlanDir {
    /// Both places, in the order they are looked at.
    pub const ALL: [PlanDir; 2] = [PlanDir::Specs, PlanDir::Plans];

    /// The folder's name in the repository.
    pub fn name(self) -> &'static str {
        match self {
            Self::Specs => SPECS_DIR,
            Self::Plans => PLANS_DIR,
        }
    }

    /// Where an entry's plan text is: `tasks.md` inside a spec folder; a chain file is itself.
    pub fn plan_file(self) -> Option<&'static str> {
        match self {
            Self::Specs => Some(TASKS_FILE),
            Self::Plans => None,
        }
    }
}

/// What kind of plan an entry is.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanKind {
    /// A folder with a `tasks.md`; it becomes one chain.
    SpecFolder,
    /// A TOML chain file.
    ChainFile,
}

/// A plan found in a project's repository.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundPlan {
    pub kind: PlanKind,
    /// The folder's name, or the file's without `.toml`.
    pub name: String,
    /// Absolute path: what `POST /v1/projects/{project}/import-path` takes, and what a chain
    /// imported from it records as its `source`.
    pub path: String,
    /// Relative to the repository, with `/`: `specs/001-login`, `plans/phase-1.toml`.
    pub relative: String,
    /// Cards an import would make (phases with tasks, or tickets); `null` when the file does not
    /// parse.
    pub cards: Option<usize>,
    /// A chain of the project was imported from this path.
    pub imported: bool,
}

/// `GET /v1/projects/{project}/plans`: the plans of the project's repository.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanScan {
    /// Sorted by `relative`.
    pub plans: Vec<FoundPlan>,
    /// The repository has a `.specify/` folder.
    pub specify_dir: bool,
    /// The plans are read from the working copy; when its branch is behind its upstream they may
    /// be older than the upstream's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behind: Option<WorkingCopyBehind>,
}

/// The working copy's branch is behind its upstream (as of the last fetch).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingCopyBehind {
    pub branch: String,
    /// `origin/main`.
    pub upstream: String,
    pub behind: u32,
}

impl WorkingCopyBehind {
    /// "main is 133 commits behind origin/main".
    pub fn note(&self) -> String {
        let commits = if self.behind == 1 {
            "commit"
        } else {
            "commits"
        };
        format!(
            "{} is {} {commits} behind {}",
            self.branch, self.behind, self.upstream
        )
    }
}

/// What the host found where an entry's plan text should be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    /// No such file.
    Missing,
    /// A file that could not be read as UTF-8 text, or too big to read.
    Unreadable,
    Text(String),
}

/// What importing a plan makes: cards, subtasks, and cards whose work is already done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    pub cards: usize,
    pub tasks: usize,
    pub done_cards: usize,
}

impl Counts {
    /// The counts of a parsed chain file.
    pub fn of_chain_file(plan: &Plan) -> Self {
        Self {
            cards: plan.tickets.len(),
            tasks: plan.tickets.iter().map(|t| t.subtasks.len()).sum(),
            done_cards: plan.tickets.iter().filter(|t| t.done).count(),
        }
    }
}

fn hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// Is the entry worth reading: a visible folder in `specs/`, a visible `.toml` file in `plans/`.
pub fn wanted(dir: PlanDir, name: &str, is_dir: bool) -> bool {
    !hidden(name)
        && match dir {
            PlanDir::Specs => is_dir,
            PlanDir::Plans => !is_dir && name.ends_with(".toml") && name.len() > ".toml".len(),
        }
}

/// The plan at an entry of `dir`, or `None` when it is not one: a spec folder needs its
/// `tasks.md`. `content` is that file's (or the chain file's) text; `sources` are the paths the
/// project's chains were imported from.
pub fn found(
    dir: PlanDir,
    name: &str,
    path: &str,
    is_dir: bool,
    content: &Content,
    sources: &[String],
) -> Option<FoundPlan> {
    if !wanted(dir, name, is_dir) || *content == Content::Missing {
        return None;
    }
    let text = match content {
        Content::Text(t) => Some(t.as_str()),
        _ => None,
    };
    let (kind, plan_name, cards) = match dir {
        PlanDir::Specs => (
            PlanKind::SpecFolder,
            name,
            text.and_then(|t| crate::spec::import(t, name, None, &[]).ok())
                .map(|i| i.cards),
        ),
        PlanDir::Plans => (
            PlanKind::ChainFile,
            name.trim_end_matches(".toml"),
            text.and_then(|t| crate::plan::parse(t).ok())
                .map(|p| p.tickets.len()),
        ),
    };
    Some(FoundPlan {
        kind,
        name: plan_name.to_owned(),
        path: path.to_owned(),
        relative: format!("{}/{name}", dir.name()),
        cards,
        imported: sources.iter().any(|s| same_path(s, path)),
    })
}

/// The answer of a scan: the plans sorted by their relative path.
pub fn scan(mut plans: Vec<FoundPlan>, specify_dir: bool) -> PlanScan {
    plans.sort_by(|a, b| a.relative.cmp(&b.relative));
    PlanScan {
        plans,
        specify_dir,
        behind: None,
    }
}

/// Do two absolute paths name the same place? A trailing separator does not count; the host
/// resolves both (links, `..`) before asking.
pub fn same_path(a: &str, b: &str) -> bool {
    let trim = |p: &str| {
        let t = p.trim_end_matches(['/', '\\']);
        if t.is_empty() {
            p.to_owned()
        } else {
            t.to_owned()
        }
    };
    trim(a) == trim(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TASKS: &str = "## Phase 1: Setup\n- [x] T001 a\n\n## Phase 2: Build\n- [ ] T002 b\n";
    const CHAIN: &str = "[chain]\nname = \"p\"\n\n[[ticket]]\nid = \"A\"\ntitle = \"A\"\nsubtasks = [\"x\", \"y\"]\ndone = true\n\n[[ticket]]\nid = \"B\"\ntitle = \"B\"\nafter = [\"A\"]\n";

    fn text(t: &str) -> Content {
        Content::Text(t.to_owned())
    }

    #[test]
    fn only_visible_folders_in_specs_and_toml_files_in_plans_are_worth_reading() {
        assert!(wanted(PlanDir::Specs, "001-login", true));
        assert!(!wanted(PlanDir::Specs, "001-login", false));
        assert!(!wanted(PlanDir::Specs, ".drafts", true));
        assert!(wanted(PlanDir::Plans, "phase-1.toml", false));
        assert!(!wanted(PlanDir::Plans, "phase-1.toml", true));
        assert!(!wanted(PlanDir::Plans, "notes.md", false));
        assert!(!wanted(PlanDir::Plans, ".toml", false));
        assert!(!wanted(PlanDir::Plans, ".old.toml", false));
    }

    #[test]
    fn a_spec_folder_needs_its_tasks_file_and_counts_its_phases() {
        let none = found(
            PlanDir::Specs,
            "001-x",
            "/r/specs/001-x",
            true,
            &Content::Missing,
            &[],
        );
        assert_eq!(none, None);
        let p = found(
            PlanDir::Specs,
            "001-x",
            "/r/specs/001-x",
            true,
            &text(TASKS),
            &[],
        )
        .expect("a spec folder");
        assert_eq!(
            (p.kind, p.name.as_str(), p.relative.as_str(), p.cards),
            (PlanKind::SpecFolder, "001-x", "specs/001-x", Some(2))
        );
        assert!(!p.imported);
        let unreadable = found(
            PlanDir::Specs,
            "002-y",
            "/r/specs/002-y",
            true,
            &Content::Unreadable,
            &[],
        )
        .expect("still a spec folder");
        assert_eq!(unreadable.cards, None);
        let broken = found(
            PlanDir::Specs,
            "003-z",
            "/r/specs/003-z",
            true,
            &text("- [q] T1 x\n"),
            &[],
        );
        assert_eq!(broken.expect("a spec folder").cards, None);
    }

    #[test]
    fn a_chain_file_is_named_after_its_file_and_counts_its_tickets() {
        let p = found(
            PlanDir::Plans,
            "phase-1.toml",
            "/r/plans/phase-1.toml",
            false,
            &text(CHAIN),
            &[],
        )
        .expect("a chain file");
        assert_eq!(
            (p.kind, p.name.as_str(), p.relative.as_str(), p.cards),
            (
                PlanKind::ChainFile,
                "phase-1",
                "plans/phase-1.toml",
                Some(2)
            )
        );
        let bad = found(
            PlanDir::Plans,
            "x.toml",
            "/r/plans/x.toml",
            false,
            &text("[[["),
            &[],
        );
        assert_eq!(bad.expect("a chain file").cards, None);
        let counts = Counts::of_chain_file(&crate::plan::parse(CHAIN).expect("parses"));
        assert_eq!(
            counts,
            Counts {
                cards: 2,
                tasks: 2,
                done_cards: 1
            }
        );
    }

    #[test]
    fn a_plan_is_imported_when_a_chain_came_from_the_same_path() {
        let sources = vec!["/r/specs/001-x/".to_owned(), "/r/plans/a.toml".to_owned()];
        let at = |dir, name: &str, path: &str, is_dir| {
            found(dir, name, path, is_dir, &text(TASKS), &sources)
                .expect("a plan")
                .imported
        };
        assert!(at(PlanDir::Specs, "001-x", "/r/specs/001-x", true));
        assert!(!at(PlanDir::Specs, "001-xy", "/r/specs/001-xy", true));
        assert!(at(PlanDir::Plans, "a.toml", "/r/plans/a.toml", false));
        assert!(!at(PlanDir::Plans, "b.toml", "/r/plans/b.toml", false));
    }

    #[test]
    fn paths_match_without_trailing_separators_and_nothing_else() {
        assert!(same_path("/r/specs/x", "/r/specs/x/"));
        assert!(same_path("C:\\r\\specs\\x\\", "C:\\r\\specs\\x"));
        assert!(same_path("/", "/"));
        assert!(!same_path("/r/specs/x", "/r/specs/x2"));
        assert!(!same_path("/r/specs/x", "/r/Specs/x"));
    }

    #[test]
    fn a_scan_lists_plans_by_relative_path() {
        let plan = |dir, name: &str| {
            found(
                dir,
                name,
                &format!("/r/{name}"),
                dir == PlanDir::Specs,
                &text(TASKS),
                &[],
            )
            .expect("a plan")
        };
        let s = scan(
            vec![
                plan(PlanDir::Specs, "002-b"),
                plan(PlanDir::Plans, "a.toml"),
                plan(PlanDir::Specs, "001-a"),
            ],
            true,
        );
        let order: Vec<&str> = s.plans.iter().map(|p| p.relative.as_str()).collect();
        assert_eq!(order, ["plans/a.toml", "specs/001-a", "specs/002-b"]);
        assert!(s.specify_dir);
    }
}
