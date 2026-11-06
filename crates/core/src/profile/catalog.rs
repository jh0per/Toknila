//! The profiles Toknila knows: the built-in ones, then one per file of the profiles folder.

use serde::{Deserialize, Serialize};

use super::parse::parse;
use crate::AgentProfile;

/// One file of the profiles folder as the adapter read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileFile {
    /// File name inside the folder.
    pub file: String,
    /// The text, or why it could not be read.
    pub text: Result<String, String>,
}

/// A profile and where it comes from.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedProfile {
    pub profile: AgentProfile,
    /// File name in the profiles folder; `None` for a built-in profile.
    pub file: Option<String>,
}

/// A profile file that cannot be used. `line` and `column` are 1-based.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileProblem {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub message: String,
}

/// Every usable profile, and the files that were not usable.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub profiles: Vec<ListedProfile>,
    pub problems: Vec<ProfileProblem>,
}

fn builtin() -> [AgentProfile; 2] {
    [AgentProfile::sim(), AgentProfile::claude_code()]
}

/// Built-in profiles first, then the valid files by name. Files that do not end in `.toml` are
/// ignored; a file named like a built-in profile is a problem, not an override.
pub fn catalog(mut files: Vec<ProfileFile>) -> Catalog {
    files.sort_by(|a, b| a.file.cmp(&b.file));
    let mut profiles: Vec<ListedProfile> = builtin()
        .into_iter()
        .map(|profile| ListedProfile {
            profile,
            file: None,
        })
        .collect();
    let mut problems = Vec::new();
    for f in files {
        let Some(name) = f.file.strip_suffix(".toml") else {
            continue;
        };
        let problem = |line, column, message: String| ProfileProblem {
            file: f.file.clone(),
            line,
            column,
            message,
        };
        let parsed = match &f.text {
            Err(e) => Err(problem(1, 1, format!("cannot read the file: {e}"))),
            Ok(_) if profiles.iter().any(|p| p.profile.name == name) => Err(problem(
                1,
                1,
                format!("{name:?} is a built-in profile; use another file name"),
            )),
            Ok(text) => parse(name, text).map_err(|e| {
                let at = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
                problem(at(e.line), at(e.column), e.message)
            }),
        };
        match parsed {
            Ok(profile) => profiles.push(ListedProfile {
                profile,
                file: Some(f.file),
            }),
            Err(p) => problems.push(p),
        }
    }
    Catalog { profiles, problems }
}
