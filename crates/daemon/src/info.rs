//! `daemon.json`: how clients find and authenticate to the running daemon.

use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Written by the daemon once it listens; removed on clean shutdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonInfo {
    pub pid: u32,
    pub port: u16,
    pub token: String,
    pub version: String,
    pub started_at: i64,
    /// Path of the daemon binary; absent in files written by versions before 0.2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe: Option<String>,
}

impl DaemonInfo {
    /// Base URL of the API.
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Reads `daemon.json`; `None` if it does not exist.
    pub fn read(path: &Path) -> std::io::Result<Option<DaemonInfo>> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Writes atomically (temp file + rename), readable only by the owner on Unix.
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        let tmp = path.with_extension("json.tmp");
        {
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut f = opts.open(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, path)
    }
}

/// A fresh random bearer token (256 bits, hex).
pub fn new_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("no OS randomness: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
