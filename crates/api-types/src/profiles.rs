//! The agent profile list.

use serde::{Deserialize, Serialize};
use toknila_core::profile::Catalog;
use toknila_core::profile::ListedProfile;
use toknila_core::profile::ProfileProblem;

/// The profiles folder, the usable profiles and the files that were not.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileList {
    /// Where profile files go.
    pub folder: String,
    pub profiles: Vec<ListedProfile>,
    pub problems: Vec<ProfileProblem>,
}

impl ProfileList {
    /// The catalog of the profiles folder `folder`.
    pub fn new(folder: String, catalog: Catalog) -> Self {
        Self {
            folder,
            profiles: catalog.profiles,
            problems: catalog.problems,
        }
    }
}
