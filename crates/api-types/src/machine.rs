//! Body of the machine-wide settings of the daemon.

use serde::{Deserialize, Serialize};

/// `GET` and `PUT /v1/machine/settings`: what the daemon does on this machine, whichever the
/// project.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineSettings {
    /// How many hand-in checks may run at once (at least 1; the default 1). A further one
    /// starts only while the one-minute load average is below the number of CPU cores.
    pub max_checks: u32,
}
