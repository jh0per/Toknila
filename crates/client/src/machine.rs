//! The machine settings routes.

use toknila_api_types::MachineSettings;

use crate::{Client, ClientResult};

impl Client {
    /// `GET /machine/settings`: how many checks may run at once.
    pub fn machine_settings(&self) -> ClientResult<MachineSettings> {
        self.call("GET", "/machine/settings", crate::NO_BODY)
    }

    /// `PUT /machine/settings`: sets how many checks may run at once.
    pub fn set_machine_settings(&self, s: &MachineSettings) -> ClientResult<MachineSettings> {
        self.call("PUT", "/machine/settings", Some(s))
    }
}
