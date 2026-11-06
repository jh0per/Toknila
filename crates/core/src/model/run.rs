use serde::{Deserialize, Serialize};

use super::{BlockedReason, RunId, TicketId, TicketKey};

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
        let detail = match (&self.signal, self.code) {
            (Some(signal), _) => format!("killed by signal {signal}"),
            (None, Some(0)) => "exited without a finished event".to_owned(),
            (None, Some(code)) => format!("exit {code}"),
            (None, None) => "exited with an unknown status".to_owned(),
        };
        BlockedReason::Crashed { detail }
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
        assert_eq!(detail(Some(1), Some("Killed")), "killed by signal Killed");
        assert_eq!(detail(None, None), "exited with an unknown status");
    }

    #[test]
    fn a_clean_exit_without_finished_is_a_crash_that_says_so() {
        assert_eq!(detail(Some(0), None), "exited without a finished event");
    }
}
