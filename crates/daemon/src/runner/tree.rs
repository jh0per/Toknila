//! The agent's process tree: does it have live children of its own (a build, tests), and how to
//! end the whole tree. Listing processes is `sysinfo` (every OS); the rule over the list is pure.

use std::collections::HashSet;
use std::path::Path;
#[cfg(unix)]
use std::time::Duration;

use sysinfo::{ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System, UpdateKind};

/// One process, as far as the rule needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Proc {
    pub pid: u32,
    pub parent: Option<u32>,
    /// File name of its executable.
    pub exe: Option<String>,
    pub zombie: bool,
}

/// Whether `root` has a live descendant that is not one of Toknila's own helpers (`helper` is the
/// `toknila` binary the agent runs for its MCP server and hooks, with everything they start).
pub(super) fn has_busy_descendant(procs: &[Proc], root: u32, helper: &str) -> bool {
    let mut seen: HashSet<u32> = HashSet::from([root]);
    let mut queue = vec![root];
    while let Some(parent) = queue.pop() {
        for p in procs.iter().filter(|p| p.parent == Some(parent)) {
            if !seen.insert(p.pid) || p.exe.as_deref() == Some(helper) {
                continue;
            }
            if !p.zombie {
                return true;
            }
            queue.push(p.pid);
        }
    }
    false
}

/// Every process on the machine now.
pub(super) fn snapshot() -> Vec<Proc> {
    let Ok(mut sys) = System::new() else {
        tracing::warn!("cannot list processes; an agent's children are not seen");
        return Vec::new();
    };
    let kind = ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet);
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
    sys.processes()
        .values()
        .map(|p| Proc {
            pid: p.pid().as_u32(),
            parent: p.parent().map(sysinfo::Pid::as_u32),
            exe: p
                .exe()
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned()),
            zombie: matches!(p.status(), ProcessStatus::Zombie | ProcessStatus::Dead),
        })
        .collect()
}

/// Whether the agent with process id `pid` has a live child of its own now.
pub(super) fn busy(pid: u32, helper: &Path) -> bool {
    let helper = helper
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    has_busy_descendant(&snapshot(), pid, &helper)
}

/// How long the tree has to end after it is asked to, before it is forced.
#[cfg(unix)]
const GRACE: Duration = Duration::from_secs(3);

/// Ends the agent's whole process tree. The agent leads its own process group (the terminal
/// makes it a session leader), so on Unix the group gets TERM, and KILL after [`GRACE`] if any of
/// it is left. Returns whether the signal could be sent; the caller then falls back to killing
/// the agent alone.
#[cfg(unix)]
pub(super) fn terminate(pid: u32) -> bool {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;
    let Ok(raw) = i32::try_from(pid) else {
        return false;
    };
    let group = Pid::from_raw(raw);
    if killpg(group, Signal::SIGTERM).is_err() {
        return false;
    }
    std::thread::spawn(move || {
        let until = std::time::Instant::now() + GRACE;
        while std::time::Instant::now() < until {
            // Signal 0 only asks whether the group still has a member.
            if killpg(group, None).is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = killpg(group, Signal::SIGKILL);
    });
    true
}

/// Ends the agent's whole process tree: `taskkill /T` walks the tree from the agent down. (A Job
/// Object would also catch orphans, but needs FFI that this workspace keeps to one audited file.)
#[cfg(windows)]
pub(super) fn terminate(pid: u32) -> bool {
    use std::process::{Command, Stdio};
    Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, parent: u32, exe: &str) -> Proc {
        Proc {
            pid,
            parent: Some(parent),
            exe: Some(exe.to_owned()),
            zombie: false,
        }
    }

    #[test]
    fn a_grandchild_counts_as_a_live_child() {
        let procs = [p(2, 1, "sh"), p(3, 2, "cargo"), p(9, 8, "other")];
        assert!(has_busy_descendant(&procs, 1, "tok"));
        assert!(!has_busy_descendant(&procs, 3, "tok"), "cargo has none");
    }

    #[test]
    fn toknilas_own_helpers_and_what_they_start_do_not_count() {
        let procs = [p(2, 1, "tok"), p(3, 2, "node")];
        assert!(!has_busy_descendant(&procs, 1, "tok"));
    }

    #[test]
    fn a_zombie_is_not_alive() {
        let mut zombie = p(2, 1, "make");
        zombie.zombie = true;
        assert!(!has_busy_descendant(&[zombie], 1, "tok"));
    }

    #[test]
    fn the_snapshot_sees_this_process_under_its_parent() {
        let me = std::process::id();
        let procs = snapshot();
        let mine = procs.iter().find(|p| p.pid == me).expect("self is listed");
        let parent = mine.parent.expect("a test process has a parent");
        assert!(has_busy_descendant(&procs, parent, "no-such-helper"));
    }
}
