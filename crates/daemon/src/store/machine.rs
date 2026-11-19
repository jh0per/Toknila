//! The machine-wide settings of the daemon that the store keeps (`app_settings`): how many
//! hand-in checks may run at once.

use toknila_api_types::SoundSettings;
use toknila_core::Actor;

use super::{Store, StoreError};

/// The `app_settings` key of the number of checks that run at once.
pub(super) const MAX_CHECKS: &str = "max_checks";

/// The `app_settings` keys of the three sound switches (`"true"` or `"false"`; absent is off).
const SOUND_KEYS: [&str; 3] = ["sound_story_done", "sound_epic_done", "sound_problem"];

impl Store {
    /// Which sounds the window plays; all off until a person turns one on.
    pub async fn sounds(&self) -> Result<SoundSettings, StoreError> {
        self.read(|repo| {
            let on = |key: &str| -> Result<bool, StoreError> {
                Ok(repo.app_setting(key)?.as_deref() == Some("true"))
            };
            Ok(SoundSettings {
                story_done: on(SOUND_KEYS[0])?,
                epic_done: on(SOUND_KEYS[1])?,
                problem: on(SOUND_KEYS[2])?,
            })
        })
        .await
    }

    /// Saves the three sound switches.
    pub async fn set_sounds(&self, s: SoundSettings) -> Result<SoundSettings, StoreError> {
        self.write(
            Actor::Human,
            crate::server::now(),
            move |tx| -> Result<(), StoreError> {
                for (key, on) in SOUND_KEYS.iter().zip([s.story_done, s.epic_done, s.problem]) {
                    tx.repo().app_setting_set(key, if on { "true" } else { "false" })?;
                }
                Ok(())
            },
        )
        .await?;
        Ok(s)
    }

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
