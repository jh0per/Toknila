//! Connection to the local daemon: starts it when it is not running.

use anyhow::Result;
use toknila_client::Client;
use toknila_daemon::paths::Paths;

use crate::daemon_cmd;

/// Connects to the daemon of this home, starting it first if needed.
pub fn connect(paths: &Paths) -> Result<Client> {
    let info = daemon_cmd::start(paths)?;
    Ok(Client::new(info.base_url(), info.token))
}
