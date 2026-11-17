//! The machine-wide settings of the daemon that the store keeps (`app_settings`): how many
//! hand-in checks may run at once.

use toknila_core::Actor;

use super::{Store, StoreError};

/// The `app_settings` key of the number of checks that run at once.
pub(super) const MAX_CHECKS: &str = "max_checks";

impl Store {
    /// How many checks may run at once on this machine (default 1).
    pub fn max_checks(&self) -> u32 {
        self.line.max_checks()
    }

    /// Saves how many checks may run at once (at least 1) and applies it to the line now.
    pub async fn set_max_checks(&self, n: u32) -> Result<u32, StoreError> {
        let n = n.max(1);
        self.write(
            Actor::Human,
            crate::server::now(),
            move |tx| -> Result<(), StoreError> {
                tx.repo().app_setting_set(MAX_CHECKS, &n.to_string())
            },
        )
        .await?;
        self.line.set_max_checks(n);
        Ok(n)
    }
}
