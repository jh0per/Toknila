//! What the app does about a daemon that is already running. After a rebuild and reinstall the old
//! daemon (older version, or the binary of a replaced app) must give way to the app's own; a
//! daemon of the same version from the same binary is simply attached to.

/// The daemon found by `daemon status --json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub version: Option<String>,
    pub exe: Option<String>,
}

/// What to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// No daemon runs: start ours.
    Start,
    /// The same version from the same binary: use it.
    Attach,
    /// Another version or binary (or one that does not say): stop it, then start ours.
    Replace,
}

/// Decides from what is running and the app's own version and daemon binary (both paths already
/// resolved the same way). A daemon that reports no binary (older than 0.2) is replaced.
pub fn decide(found: Option<&Found>, own_version: &str, own_exe: &str) -> Decision {
    let Some(f) = found else {
        return Decision::Start;
    };
    if f.version.as_deref() == Some(own_version) && f.exe.as_deref() == Some(own_exe) {
        Decision::Attach
    } else {
        Decision::Replace
    }
}

/// Reads the answer of `toknila daemon status --json`: `None` when no daemon runs.
pub fn parse_status(json: &str) -> Option<Found> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    if v["running"].as_bool() != Some(true) {
        return None;
    }
    let text = |k: &str| v[k].as_str().map(str::to_owned);
    Some(Found {
        version: text("version"),
        exe: text("exe"),
    })
}

/// The message in the window when the old daemon would not stop.
pub fn stuck_message(found: &Found, reason: &str) -> String {
    format!(
        "An older Toknila daemon (version {}, {}) is still running and did not stop: {reason}\n\
         Stop it with `tok daemon stop` (or quit its process) and open Toknila again.",
        found.version.as_deref().unwrap_or("unknown"),
        found.exe.as_deref().unwrap_or("unknown binary"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(v: Option<&str>, e: Option<&str>) -> Found {
        Found {
            version: v.map(str::to_owned),
            exe: e.map(str::to_owned),
        }
    }

    #[test]
    fn nothing_running_starts_ours() {
        assert_eq!(decide(None, "0.2.0", "/a/toknila"), Decision::Start);
    }

    #[test]
    fn same_version_and_binary_is_attached() {
        let f = found(Some("0.2.0"), Some("/a/toknila"));
        assert_eq!(decide(Some(&f), "0.2.0", "/a/toknila"), Decision::Attach);
    }

    #[test]
    fn another_version_another_path_or_an_unknown_one_is_replaced() {
        let own = ("0.2.0", "/a/toknila");
        for f in [
            found(Some("0.1.0"), Some("/a/toknila")),
            found(Some("0.2.0"), Some("/b/toknila")),
            found(Some("0.2.0"), None),
            found(None, Some("/a/toknila")),
        ] {
            assert_eq!(decide(Some(&f), own.0, own.1), Decision::Replace, "{f:?}");
        }
    }

    #[test]
    fn status_json_is_read_with_and_without_a_binary() {
        let on =
            r#"{"running":true,"pid":1,"port":2,"version":"0.2.0","exe":"/a/toknila","home":"/h"}"#;
        assert_eq!(
            parse_status(on),
            Some(found(Some("0.2.0"), Some("/a/toknila")))
        );
        let old = r#"{"running":true,"pid":1,"port":2,"version":"0.1.0","home":"/h"}"#;
        assert_eq!(parse_status(old), Some(found(Some("0.1.0"), None)));
        assert_eq!(parse_status(r#"{"running":false,"home":"/h"}"#), None);
        assert_eq!(parse_status("garbage"), None);
    }

    #[test]
    fn the_message_names_the_old_daemon_and_the_way_out() {
        let m = stuck_message(&found(Some("0.1.0"), None), "timed out");
        assert!(m.contains("version 0.1.0") && m.contains("unknown binary"));
        assert!(m.contains("tok daemon stop") && m.contains("timed out"));
    }
}
