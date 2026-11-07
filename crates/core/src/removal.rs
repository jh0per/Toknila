//! Deleting a project: what may become of its repository folder. The records go in one
//! transaction; afterwards the host may move the folder to the system Trash, never delete it,
//! and only when [`may_trash`] allows it. Pure: the host resolves the paths (links, `..`) and
//! says what is at the folder's path.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::repo::PathKind;

/// What became of a deleted project's repository folder.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderFate {
    /// It stayed where it was: only the records were deleted, or the Trash refused or failed.
    Kept,
    /// It is in the system Trash.
    Trashed,
    /// The project had no folder, or it was no longer there.
    None,
}

/// Why a folder may not go to the Trash.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TrashRefusal {
    #[error("{0} is not an absolute path")]
    NotAbsolute(String),
    #[error("{0} does not exist")]
    Missing(String),
    #[error("{0} is not a folder")]
    NotAFolder(String),
    #[error("{0} is the root of a disk")]
    Root(String),
    #[error("{0} holds Toknila's own data or is inside it")]
    DataHome(String),
    #[error("{0} is your home folder or a folder above it")]
    UserHome(String),
}

impl TrashRefusal {
    /// The folder's fate after this refusal: one that is not there is none, any other stays.
    pub fn fate(&self) -> FolderFate {
        match self {
            Self::Missing(_) => FolderFate::None,
            _ => FolderFate::Kept,
        }
    }
}

/// May `folder` go to the Trash? Only an absolute path to an existing folder that is not the
/// root of a disk, does not hold Toknila's data home (`data_home`) and is not inside it, and is
/// neither the person's home folder (`user_home`) nor a folder above it.
pub fn may_trash(
    folder: &Path,
    kind: &PathKind,
    data_home: &Path,
    user_home: Option<&Path>,
) -> Result<(), TrashRefusal> {
    let text = folder.display().to_string();
    if !folder.is_absolute() {
        return Err(TrashRefusal::NotAbsolute(text));
    }
    match kind {
        PathKind::Missing => return Err(TrashRefusal::Missing(text)),
        PathKind::File => return Err(TrashRefusal::NotAFolder(text)),
        PathKind::Dir => {}
    }
    if folder.parent().is_none() {
        return Err(TrashRefusal::Root(text));
    }
    if folder.starts_with(data_home) || data_home.starts_with(folder) {
        return Err(TrashRefusal::DataHome(text));
    }
    if user_home.is_some_and(|home| home.starts_with(folder)) {
        return Err(TrashRefusal::UserHome(text));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// An absolute path on this platform.
    fn abs(rel: &str) -> PathBuf {
        let root = if cfg!(windows) { "C:\\" } else { "/" };
        Path::new(root).join(rel)
    }

    fn judge(folder: &Path, kind: PathKind) -> Result<(), TrashRefusal> {
        let (data, home) = (abs("home/me/.toknila"), abs("home/me"));
        may_trash(folder, &kind, &data, Some(&home))
    }

    #[test]
    fn a_project_folder_under_the_home_folder_may_go() {
        assert_eq!(judge(&abs("home/me/work/app"), PathKind::Dir), Ok(()));
        assert_eq!(judge(&abs("srv/repos/app"), PathKind::Dir), Ok(()));
    }

    #[test]
    fn only_an_absolute_path_to_an_existing_folder() {
        assert!(matches!(
            judge(Path::new("work/app"), PathKind::Dir),
            Err(TrashRefusal::NotAbsolute(_))
        ));
        let gone = judge(&abs("home/me/work/gone"), PathKind::Missing);
        assert!(matches!(gone, Err(TrashRefusal::Missing(_))));
        assert_eq!(gone.expect_err("missing").fate(), FolderFate::None);
        let file = judge(&abs("home/me/notes.txt"), PathKind::File);
        assert!(matches!(file, Err(TrashRefusal::NotAFolder(_))));
        assert_eq!(file.expect_err("a file").fate(), FolderFate::Kept);
    }

    #[test]
    fn never_the_root_of_a_disk() {
        let root = abs("");
        assert!(matches!(
            judge(root.ancestors().last().expect("root"), PathKind::Dir),
            Err(TrashRefusal::Root(_))
        ));
    }

    #[test]
    fn never_the_data_home_what_is_inside_it_or_what_holds_it() {
        for folder in [
            "home/me/.toknila",
            "home/me/.toknila/ships/demo/auth/demo-1",
        ] {
            assert!(
                matches!(
                    judge(&abs(folder), PathKind::Dir),
                    Err(TrashRefusal::DataHome(_))
                ),
                "{folder}"
            );
        }
        // A data home kept inside a repository (TOKNILA_HOME) protects the repository too.
        let data = abs("srv/app/.tk");
        assert!(matches!(
            may_trash(&abs("srv/app"), &PathKind::Dir, &data, None),
            Err(TrashRefusal::DataHome(_))
        ));
    }

    #[test]
    fn never_the_home_folder_or_a_folder_above_it() {
        let (data, home) = (abs("var/toknila"), abs("home/me"));
        for folder in ["home/me", "home"] {
            assert!(
                matches!(
                    may_trash(&abs(folder), &PathKind::Dir, &data, Some(&home)),
                    Err(TrashRefusal::UserHome(_))
                ),
                "{folder}"
            );
        }
        assert_eq!(
            may_trash(&abs("home/me"), &PathKind::Dir, &abs("var/toknila"), None),
            Ok(()),
            "without a known home folder only the other rules apply"
        );
    }
}
