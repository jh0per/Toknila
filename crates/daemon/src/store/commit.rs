//! A command's transaction: the commit, and what follows it on the writer thread.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

use toknila_app::external::ExternalEffect;
use toknila_core::{Actor, Timestamp};

use super::{EventRecord, Store, StoreError, WriteTx, panic_message};

/// A test seam: what runs on the writer thread right after a commit.
#[derive(Clone, Default)]
pub(super) struct Hook(Arc<Mutex<Option<Action>>>);

/// What a hook runs.
pub type Action = Arc<dyn Fn() + Send + Sync>;

impl std::fmt::Debug for Hook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Hook")
    }
}

impl Store {
    /// Runs `f` as one transaction. On `Ok` the state and the emitted events commit together and
    /// the events are published; on `Err` or panic nothing is written. Work the command queued
    /// for the host is started on the writer thread in the same step as the commit, in a task
    /// the request does not own (see [`Store::spawn_effects`]): a request that is dropped at any
    /// point after the job was sent still gets them run. Wait with `settled`.
    pub async fn write<T, E, F>(
        &self,
        actor: Actor,
        now: Timestamp,
        f: F,
    ) -> Result<(T, Vec<EventRecord>), E>
    where
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
        F: FnOnce(&mut WriteTx<'_>) -> Result<T, E> + Send + 'static,
    {
        let (value, emitted, _) = self.commit_with_effects(actor, now, true, f).await?;
        Ok((value, emitted))
    }

    /// Runs `f` in one transaction. With `detach` the queued effects are spawned on the writer
    /// thread right after the commit and none are returned; without it they are returned to the
    /// caller, which is already inside an effect task and runs them itself.
    pub(super) async fn commit_with_effects<T, E, F>(
        &self,
        actor: Actor,
        now: Timestamp,
        detach: bool,
        f: F,
    ) -> Result<(T, Vec<EventRecord>, Vec<ExternalEffect>), E>
    where
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
        F: FnOnce(&mut WriteTx<'_>) -> Result<T, E> + Send + 'static,
    {
        let events = self.events.clone();
        let live = self.runner.live_runners();
        let line = self.line.clone();
        let store = self.clone();
        let runtime = tokio::runtime::Handle::current();
        let outcome = self
            .run(move |conn| {
                let tx = match conn.transaction() {
                    Ok(tx) => tx,
                    Err(e) => return Ok(Err(E::from(e.into()))),
                };
                let mut w = WriteTx {
                    tx,
                    actor,
                    now,
                    events: Vec::new(),
                    deferred: Vec::new(),
                    live,
                };
                let value = match catch_unwind(AssertUnwindSafe(|| f(&mut w))) {
                    Ok(Ok(v)) => v,
                    Ok(Err(e)) => return Ok(Err(e)),
                    Err(p) => return Ok(Err(E::from(StoreError::Panicked(panic_message(&p))))),
                };
                let WriteTx {
                    tx,
                    events: emitted,
                    mut deferred,
                    ..
                } = w;
                if let Err(e) = tx.commit() {
                    return Ok(Err(E::from(e.into())));
                }
                // The checks take their places in the line here, on the writer thread right
                // after the commit: the line holds the hand-ins in the order they committed,
                // whichever effect task reaches the host first.
                for effect in &deferred {
                    if let ExternalEffect::RunCheck { ticket, .. } = effect {
                        line.join(*ticket);
                    }
                }
                // The effects start here too, counted before this job returns, so no await
                // point of the request can lose them and a `settled` after the command sees them.
                if detach {
                    store.spawn_effects(&runtime, std::mem::take(&mut deferred));
                }
                for e in &emitted {
                    // No subscribers is fine.
                    let _ = events.send(e.clone());
                }
                store.run_hook();
                Ok(Ok((value, emitted, deferred)))
            })
            .await;
        match outcome {
            Ok(inner) => inner,
            Err(e) => Err(E::from(e)),
        }
    }

    /// Test seam: `hook` runs on the writer thread right after every commit, before the
    /// command's answer is sent. Lets a test drop the request at the earliest moment.
    #[doc(hidden)]
    pub fn on_committed(&self, hook: Option<Action>) {
        if let Ok(mut h) = self.hook.0.lock() {
            *h = hook;
        }
    }

    fn run_hook(&self) {
        let hook = self.hook.0.lock().ok().and_then(|h| h.clone());
        if let Some(h) = hook {
            h();
        }
    }
}
