//! The effect tasks of a store. After a command committed, the work it queued for the host runs
//! in a task the request does not own, so a request that is dropped (the agent's client gave up,
//! the agent was stopped by the same hand-in) never loses an outcome. This tracks those tasks:
//! tests and the shutdown wait for them.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::watch;
use toknila_app::external::ExternalEffect;

/// The running effect tasks, and whether the store is closing.
#[derive(Debug)]
pub(super) struct Tasks {
    running: watch::Sender<usize>,
    labels: Mutex<BTreeMap<u64, String>>,
    next: AtomicU64,
    closing: AtomicBool,
}

impl Tasks {
    pub(super) fn new() -> Arc<Tasks> {
        Arc::new(Tasks {
            running: watch::channel(0).0,
            labels: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(0),
            closing: AtomicBool::new(false),
        })
    }

    /// Counts a task that is about to be spawned; the count drops when the guard does. Called
    /// before the spawn, so a `settled` after the command that queued it always sees it.
    pub(super) fn start(self: &Arc<Self>, label: String) -> Guard {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut l) = self.labels.lock() {
            l.insert(id, label);
        }
        self.running.send_modify(|n| *n += 1);
        Guard {
            tasks: self.clone(),
            id,
        }
    }

    /// Resolves when no effect task is running.
    pub(super) async fn settled(&self) {
        let mut rx = self.running.subscribe();
        let _ = rx.wait_for(|n| *n == 0).await;
    }

    /// What the tasks still running are doing.
    pub(super) fn pending(&self) -> Vec<String> {
        self.labels
            .lock()
            .map(|l| l.values().cloned().collect())
            .unwrap_or_default()
    }

    pub(super) fn close(&self) {
        self.closing.store(true, Ordering::SeqCst);
    }

    pub(super) fn closing(&self) -> bool {
        self.closing.load(Ordering::SeqCst)
    }

    /// Waits up to `bound` for the tasks; returns what is still running after it.
    pub(super) async fn settled_within(&self, bound: Duration) -> Vec<String> {
        match tokio::time::timeout(bound, self.settled()).await {
            Ok(()) => Vec::new(),
            Err(_) => self.pending(),
        }
    }
}

impl super::Store {
    /// Resolves when every effect task started so far has finished and recorded its outcomes.
    /// A command's effects run detached from its caller; tests that look for an effect's result
    /// wait here instead of on the clock.
    pub async fn settled(&self) {
        self.tasks.settled().await;
    }

    /// Stops the store's host work for the daemon's end: a check still running is killed and its
    /// outcome is not recorded (the next start hands the card to a person), and the effect tasks
    /// get `bound` to finish. Returns what is still running after it, which is abandoned.
    pub async fn shutdown(&self, bound: Duration) -> Vec<String> {
        self.tasks.close();
        self.checker.stop_all();
        let abandoned = self.tasks.settled_within(bound).await;
        for what in &abandoned {
            tracing::warn!(effects = %what, "an effect was still running at shutdown and is abandoned");
        }
        abandoned
    }

    /// Runs what a command queued in a task the caller does not own.
    pub(super) fn detach_effects(&self, queued: Vec<ExternalEffect>) {
        if queued.is_empty() {
            return;
        }
        let label = queued
            .iter()
            .map(|e| {
                let d = format!("{e:?}");
                d.split([' ', '{', '('])
                    .next()
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join(", ");
        let guard = self.tasks.start(label);
        let store = self.clone();
        tokio::spawn(async move {
            store.after_commit(queued).await;
            drop(guard);
        });
    }
}

/// One counted effect task.
#[derive(Debug)]
pub(super) struct Guard {
    tasks: Arc<Tasks>,
    id: u64,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Ok(mut l) = self.tasks.labels.lock() {
            l.remove(&self.id);
        }
        self.tasks.running.send_modify(|n| *n -= 1);
    }
}
