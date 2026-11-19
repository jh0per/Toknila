//! Body of the machine-wide settings of the daemon.

use serde::{Deserialize, Serialize};

/// Which sounds the window plays; each is a switch, off until a person turns it on.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundSettings {
    /// A story is done (merged).
    pub story_done: bool,
    /// An epic is finished (its last story is done, or its release ran through).
    pub epic_done: bool,
    /// Something went wrong (a story blocked on its own, a release step failed).
    pub problem: bool,
}

/// `GET` and `PUT /v1/machine/settings`: what the daemon does on this machine, whichever the
/// project.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineSettings {
    /// How many hand-in checks may run at once (at least 1; the default 1). A further one
    /// starts only while the one-minute load average is below the number of CPU cores.
    pub max_checks: u32,
    /// The sound switches. The daemon always answers with them; a `PUT` without them (an older
    /// `tok` or page) leaves them as they are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sounds: Option<SoundSettings>,
}
