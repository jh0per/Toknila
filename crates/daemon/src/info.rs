//! `daemon.json`: how clients find and authenticate to the running daemon.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use toknila_core::address::{Address, INCOMPLETE, is_complete};

/// The pauses before an incomplete address file is given up on: the window the CLI already
/// allows a daemon that does not answer.
pub const READ_PAUSES: &[Duration] = &[Duration::from_millis(200), Duration::from_millis(200)];

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

    /// Reads `daemon.json`; `None` if it does not exist. A file that is empty, cut or malformed
    /// is an `InvalidData` error saying `the address file is incomplete`.
    pub fn read(path: &Path) -> std::io::Result<Option<DaemonInfo>> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let incomplete = || std::io::Error::new(std::io::ErrorKind::InvalidData, INCOMPLETE);
        let info: DaemonInfo = serde_json::from_slice(&bytes).map_err(|_| incomplete())?;
        let address = Address {
            port: info.port,
            token: &info.token,
        };
        if is_complete(address) {
            Ok(Some(info))
        } else {
            Err(incomplete())
        }
    }

    /// [`DaemonInfo::read`] that treats an incomplete file as "the daemon is starting": it is read
    /// again after each pause of [`READ_PAUSES`] before the error is returned.
    pub fn read_settled(path: &Path) -> std::io::Result<Option<DaemonInfo>> {
        Self::read_settled_with(path, READ_PAUSES)
    }

    /// [`DaemonInfo::read_settled`] with the pauses given, so a test can set the window instead
    /// of racing a writer thread against the two hundred milliseconds of the real one.
    fn read_settled_with(path: &Path, pauses: &[Duration]) -> std::io::Result<Option<DaemonInfo>> {
        for pause in pauses {
            match Self::read(path) {
                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                    std::thread::sleep(*pause);
                }
                other => return other,
            }
        }
        Self::read(path)
    }

    /// Writes atomically (temp file + rename), readable only by the owner on Unix.
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        // A name of its own: two daemons starting in one home never write the same temp file.
        let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> DaemonInfo {
        DaemonInfo {
            pid: 1,
            port: 4100,
            token: new_token().expect("token"),
            version: "0.0.0".into(),
            started_at: 0,
            exe: None,
        }
    }

    #[test]
    fn an_empty_cut_or_tokenless_file_is_incomplete_not_a_raw_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("daemon.json");
        let one = info();
        let full = serde_json::to_string_pretty(&one).expect("json");
        let tokenless = full.replace(&one.token, "");
        for text in [String::new(), full[..full.len() / 2].to_owned(), tokenless] {
            std::fs::write(&path, &text).expect("write");
            let err = DaemonInfo::read(&path).expect_err("incomplete");
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
            assert_eq!(err.to_string(), INCOMPLETE, "for {text:?}");
        }
    }

    #[test]
    fn a_file_completed_during_the_pauses_is_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("daemon.json");
        std::fs::write(&path, "").expect("write");
        let (p, i) = (path.clone(), info());
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            i.write(&p)
        });
        // The real pauses are 200 ms each; a loaded Windows runner once took longer than that
        // to schedule the writer, so the test owns the window: two pauses of two seconds,
        // and the read returns the moment a pause ends with the file complete.
        let read = DaemonInfo::read_settled_with(&path, &[Duration::from_secs(2); 2]);
        writer.join().expect("writer thread").expect("write");
        let read = read.expect("settles").expect("some");
        assert_eq!(read.port, 4100);
    }

    #[test]
    fn a_file_that_stays_incomplete_is_reported_after_the_pauses() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("daemon.json");
        std::fs::write(&path, "{\"pid\":1,\"po").expect("write");
        let err = DaemonInfo::read_settled(&path).expect_err("incomplete");
        assert_eq!(err.to_string(), INCOMPLETE);
    }

    #[test]
    fn no_reader_sees_an_empty_or_partial_file_across_200_restarts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("daemon.json");
        info().write(&path).expect("first");
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (p, s) = (path.clone(), stop.clone());
        let reader = std::thread::spawn(move || {
            let mut reads = 0u32;
            while !s.load(std::sync::atomic::Ordering::Relaxed) {
                let got = DaemonInfo::read(&p).expect("a complete file, never a cut one");
                assert!(got.is_some(), "the file never disappears between restarts");
                reads += 1;
            }
            reads
        });
        for _ in 0..200 {
            info().write(&path).expect("restart");
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(reader.join().expect("reader") > 0);
    }
}
