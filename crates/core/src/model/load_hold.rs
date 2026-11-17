//! Why a check that has a free place still waits for the machine.

use serde::{Deserialize, Serialize};

/// The machine's one-minute load average is at or above its core count, so a further check
/// waits.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadHold {
    /// The one-minute load average, rounded.
    pub load: u32,
    /// The number of CPU cores.
    pub cores: u32,
}

impl LoadHold {
    /// What the card says: `waits for the machine: load 41 on 16 cores`.
    pub fn words(&self) -> String {
        format!(
            "waits for the machine: load {} on {} cores",
            self.load, self.cores
        )
    }
}
