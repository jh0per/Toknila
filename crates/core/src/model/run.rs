use serde::{Deserialize, Serialize};

use super::{AgentRole, BlockedReason, RunId, TicketId, TicketKey};

/// One process the daemon ran for a ticket: its runner, from start to exit.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub ticket_id: TicketId,
    pub key: TicketKey,
    /// Name of the agent profile whose command runs.
    pub profile: String,
    /// Directory the process started in: the ticket's ship.
    pub dir: String,
    /// What the process is for: the ticket's worker, or the reviewer next to it. The worker is
    /// the default and is not written out.
    #[serde(default = "worker_role", skip_serializing_if = "is_worker")]
    pub role: AgentRole,
}

fn worker_role() -> AgentRole {
    AgentRole::Worker
}

fn is_worker(role: &AgentRole) -> bool {
    *role == AgentRole::Worker
}

/// How a runner's process ended.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessExit {
    /// Exit code, when the process ended by itself.
    pub code: Option<i32>,
    /// Name of the signal that killed it, where the platform reports one.
    pub signal: Option<String>,
}

impl ProcessExit {
    /// Why a ticket whose agent was still working is blocked by this exit. A non-zero code or a
    /// signal is a crash. A zero exit is a crash too: the agent left without reporting
    /// `finished`, so the work cannot be taken as done; unblocking starts the runner again.
    pub fn block_reason(&self) -> BlockedReason {
        if let Some(how) = self.terminated_from_outside() {
            return BlockedReason::Terminated { detail: how };
        }
        let detail = match (&self.signal, self.code) {
            (Some(signal), _) => format!("killed by signal {signal}"),
            (None, Some(0)) => "exited without a finished event".to_owned(),
            (None, Some(code)) => format!("exit {code}"),
            (None, None) => "exited with an unknown status".to_owned(),
        };
        BlockedReason::Crashed { detail }
    }
}

/// Windows' exit code of a process ended by a console control event or a kill
/// (`STATUS_CONTROL_C_EXIT`), as the signed value the platform hands back.
const WINDOWS_TERMINATED: i32 = 0xC000_013A_u32 as i32;

impl ProcessExit {
    /// How the process was ended by someone else, when it was: a terminate, kill, hang-up or
    /// interrupt signal, the shell's code for one (128 plus the signal number: 143, 137, 129, 130)
    /// or Windows' terminated status. Other signals (a segfault) are crashes of the agent.
    fn terminated_from_outside(&self) -> Option<String> {
        if let Some(signal) = &self.signal {
            let known = ["terminated", "killed", "hangup", "hang up", "interrupt"];
            let lower = signal.to_lowercase();
            return known
                .iter()
                .any(|k| lower.contains(k) || lower == "sigterm" || lower == "sigkill")
                .then(|| format!("signal {signal}"));
        }
        match self.code? {
            143 => Some("exit 143, SIGTERM".to_owned()),
            137 => Some("exit 137, SIGKILL".to_owned()),
            129 => Some("exit 129, SIGHUP".to_owned()),
            130 => Some("exit 130, SIGINT".to_owned()),
            WINDOWS_TERMINATED => Some("exit 0xC000013A, the process was terminated".to_owned()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail(code: Option<i32>, signal: Option<&str>) -> String {
        let exit = ProcessExit {
            code,
            signal: signal.map(str::to_owned),
        };
        match exit.block_reason() {
            BlockedReason::Crashed { detail } => detail,
            other => panic!("expected crashed, got {other:?}"),
        }
    }

    #[test]
    fn exit_codes_and_signals_become_crash_details() {
        assert_eq!(detail(Some(101), None), "exit 101");
        assert_eq!(
            detail(Some(1), Some("Segmentation fault")),
            "killed by signal Segmentation fault"
        );
        assert_eq!(detail(None, None), "exited with an unknown status");
    }

    fn terminated(code: Option<i32>, signal: Option<&str>) -> Option<String> {
        let exit = ProcessExit {
            code,
            signal: signal.map(str::to_owned),
        };
        match exit.block_reason() {
            BlockedReason::Terminated { detail } => Some(detail),
            _ => None,
        }
    }

    #[test]
    fn an_agent_ended_from_outside_is_terminated_not_crashed() {
        assert_eq!(terminated(Some(143), None).as_deref(), Some("exit 143, SIGTERM"));
        assert_eq!(terminated(Some(137), None).as_deref(), Some("exit 137, SIGKILL"));
        assert_eq!(terminated(None, Some("Terminated")).as_deref(), Some("signal Terminated"));
        assert_eq!(terminated(None, Some("Killed")).as_deref(), Some("signal Killed"));
        assert!(terminated(Some(-1_073_741_510), None).is_some());
    }

    #[test]
    fn real_failures_stay_crashes() {
        assert_eq!(terminated(Some(101), None), None);
        assert_eq!(terminated(Some(1), None), None);
        assert_eq!(terminated(Some(0), None), None);
        assert_eq!(terminated(None, Some("Segmentation fault")), None);
    }

    #[test]
    fn a_clean_exit_without_finished_is_a_crash_that_says_so() {
        assert_eq!(detail(Some(0), None), "exited without a finished event");
    }
}
