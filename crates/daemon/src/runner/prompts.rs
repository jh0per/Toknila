//! What the runs ask that the daemon holds in memory: the last tool each agent tried to run, the
//! permission dialogs that wait for a person, and which runs are flagged as waiting on their
//! screen. None of it is stored: a daemon restart ends every run, and the journal keeps what
//! mattered (`question_raised`, `question_answered`, `question_cleared`).

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard, PoisonError};

use tokio::sync::oneshot;
use toknila_core::question::{Decision, PendingPermission};
use toknila_core::{AgentRole, TicketId, Timestamp};

type Key = (TicketId, AgentRole);

#[derive(Debug)]
struct Pending {
    text: String,
    detail: Option<String>,
    /// The whole tool call the dialog is about (`Bash: <command>`), when it is known.
    tool: Option<String>,
    since: Timestamp,
    /// The permission hook that waits for the person's decision, if one does.
    waiter: Option<oneshot::Sender<Decision>>,
}

#[derive(Debug, Default)]
struct Inner {
    tools: HashMap<Key, String>,
    pending: HashMap<Key, Pending>,
    screens: HashSet<Key>,
}

/// The in-memory questions of the live runs.
#[derive(Debug, Default)]
pub struct Prompts {
    inner: Mutex<Inner>,
}

impl Prompts {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Remembers the tool call the run is about to make, as the hook told it (the whole text).
    pub fn remember_tool(&self, ticket: TicketId, role: AgentRole, tool: String) {
        self.inner().tools.insert((ticket, role), tool);
    }

    /// The run's last tool call, as the hook told it.
    pub fn last_tool(&self, ticket: TicketId, role: AgentRole) -> Option<String> {
        self.inner().tools.get(&(ticket, role)).cloned()
    }

    /// A permission dialog the run shows. With `hook`, the permission hook waits for the person's
    /// decision and the receiver delivers it. An open dialog is kept as it is: a notification
    /// that follows the hook's request is the same dialog. Returns `None` for a dialog without a
    /// hook, or when one is already open.
    pub fn begin(
        &self,
        ticket: TicketId,
        role: AgentRole,
        text: String,
        detail: Option<String>,
        since: Timestamp,
        hook: bool,
    ) -> (bool, Option<oneshot::Receiver<Decision>>) {
        let mut inner = self.inner();
        let key = (ticket, role);
        if inner.pending.contains_key(&key) && !hook {
            return (false, None);
        }
        let (waiter, rx) = if hook {
            let (tx, rx) = oneshot::channel();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };
        let fresh = inner
            .pending
            .insert(
                key,
                Pending {
                    text,
                    detail,
                    tool: None,
                    since,
                    waiter,
                },
            )
            .is_none();
        (fresh, rx)
    }

    /// Records the tool call the open dialog of the run is about, as the hook told it.
    pub fn note_dialog_tool(&self, ticket: TicketId, role: AgentRole, tool: String) {
        if let Some(p) = self.inner().pending.get_mut(&(ticket, role)) {
            p.tool = Some(tool);
        }
    }

    /// The tool call the open dialog of the run is about, if the dialog and the call are known.
    pub fn dialog_tool(&self, ticket: TicketId, role: AgentRole) -> Option<String> {
        self.inner().pending.get(&(ticket, role))?.tool.clone()
    }

    /// The worker dialogs a permission hook holds open, with the tool call each is about.
    pub fn worker_hook_dialogs(&self) -> Vec<(TicketId, String)> {
        let inner = self.inner();
        inner
            .pending
            .iter()
            .filter(|((_, role), p)| {
                *role == AgentRole::Worker && p.waiter.as_ref().is_some_and(|w| !w.is_closed())
            })
            .filter_map(|((ticket, _), p)| Some((*ticket, p.tool.clone()?)))
            .collect()
    }

    /// Whether a permission hook of the run waits for a decision.
    pub fn hook_waiting(&self, ticket: TicketId, role: AgentRole) -> bool {
        self.inner()
            .pending
            .get(&(ticket, role))
            .is_some_and(|p| p.waiter.as_ref().is_some_and(|w| !w.is_closed()))
    }

    /// Whether the run shows a permission dialog.
    pub fn is_pending(&self, ticket: TicketId, role: AgentRole) -> bool {
        self.inner().pending.contains_key(&(ticket, role))
    }

    /// Hands the decision to the hook that waits for it. Returns whether one did.
    pub fn resolve(&self, ticket: TicketId, role: AgentRole, decision: Decision) -> bool {
        let mut inner = self.inner();
        let key = (ticket, role);
        if inner.pending.get(&key).is_none_or(|p| p.waiter.is_none()) {
            return false;
        }
        inner
            .pending
            .remove(&key)
            .and_then(|p| p.waiter)
            .is_some_and(|w| w.send(decision).is_ok())
    }

    /// The dialog is gone (the agent went on, or the hook gave up). Returns whether one was open.
    pub fn clear(&self, ticket: TicketId, role: AgentRole) -> bool {
        self.inner().pending.remove(&(ticket, role)).is_some()
    }

    /// The open dialogs, for the inbox.
    pub fn pending(&self) -> Vec<PendingPermission> {
        self.inner()
            .pending
            .iter()
            .map(|((ticket, role), p)| PendingPermission {
                ticket: *ticket,
                role: *role,
                text: p.text.clone(),
                detail: p.detail.clone(),
                since: p.since,
            })
            .collect()
    }

    /// Flags the run as waiting on its screen. Returns whether it was not flagged before.
    pub fn flag_screen(&self, ticket: TicketId, role: AgentRole) -> bool {
        self.inner().screens.insert((ticket, role))
    }

    /// Takes the flag off. Returns whether it was set.
    pub fn unflag_screen(&self, ticket: TicketId, role: AgentRole) -> bool {
        self.inner().screens.remove(&(ticket, role))
    }

    /// The runs flagged as waiting on their screen.
    pub fn flagged(&self) -> Vec<(TicketId, AgentRole)> {
        self.inner().screens.iter().copied().collect()
    }

    /// A run starts over: nothing of the previous one applies.
    pub fn reset(&self, ticket: TicketId, role: AgentRole) {
        let mut inner = self.inner();
        let key = (ticket, role);
        inner.tools.remove(&key);
        inner.pending.remove(&key);
        inner.screens.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> (TicketId, AgentRole) {
        (TicketId::new(), AgentRole::Reviewer)
    }

    #[test]
    fn a_decision_reaches_the_waiting_hook_once() {
        let prompts = Prompts::default();
        let (t, r) = ids();
        let (fresh, rx) = prompts.begin(t, r, "Bash: ls".into(), None, Timestamp(1), true);
        assert!(fresh);
        let mut rx = rx.expect("receiver");
        assert!(prompts.hook_waiting(t, r));
        assert!(prompts.resolve(t, r, Decision::Allow));
        assert_eq!(rx.try_recv().ok(), Some(Decision::Allow));
        assert!(!prompts.is_pending(t, r));
        assert!(!prompts.resolve(t, r, Decision::Allow), "once");
    }

    #[test]
    fn a_dialog_without_a_hook_is_open_but_nothing_waits_for_it() {
        let prompts = Prompts::default();
        let (t, r) = ids();
        let (fresh, rx) = prompts.begin(t, r, "p".into(), None, Timestamp(1), false);
        assert!(fresh && rx.is_none());
        assert!(prompts.is_pending(t, r) && !prompts.hook_waiting(t, r));
        assert!(!prompts.resolve(t, r, Decision::Allow));
        assert_eq!(
            prompts.pending().len(),
            1,
            "no hook to answer: the dialog stays open"
        );
    }

    #[test]
    fn a_notification_does_not_replace_the_dialog_a_hook_holds() {
        let prompts = Prompts::default();
        let (t, r) = ids();
        let (_, rx) = prompts.begin(t, r, "from the hook".into(), None, Timestamp(1), true);
        let (fresh, none) = prompts.begin(
            t,
            r,
            "from the notification".into(),
            None,
            Timestamp(9),
            false,
        );
        assert!(!fresh && none.is_none());
        assert!(prompts.hook_waiting(t, r));
        let open = prompts.pending();
        assert_eq!(open.len(), 1);
        assert_eq!(
            (open[0].text.as_str(), open[0].since),
            ("from the hook", Timestamp(1))
        );
        drop(rx);
        assert!(
            !prompts.hook_waiting(t, r),
            "a hook that gave up waits no more"
        );
        assert!(prompts.clear(t, r));
        assert!(!prompts.clear(t, r));
    }

    #[test]
    fn the_last_tool_and_the_screen_flag_belong_to_one_role_of_one_ticket() {
        let prompts = Prompts::default();
        let (t, r) = ids();
        prompts.remember_tool(t, r, "Bash: ls".into());
        assert_eq!(prompts.last_tool(t, r).as_deref(), Some("Bash: ls"));
        assert_eq!(prompts.last_tool(t, AgentRole::Worker), None);
        assert!(prompts.flag_screen(t, r));
        assert!(!prompts.flag_screen(t, r));
        assert_eq!(prompts.flagged(), [(t, r)]);
        prompts.reset(t, r);
        assert_eq!(prompts.last_tool(t, r), None);
        assert!(prompts.flagged().is_empty());
        assert!(!prompts.unflag_screen(t, r));
    }
}
