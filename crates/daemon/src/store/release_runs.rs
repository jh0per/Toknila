//! The journal of release runs. A run's steps are events of the entity `release_run`, one entry
//! when the run starts, one when a step starts, passes or fails, and one when the run ends; the
//! run is read back by folding them, so it is the same after a daemon restart.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use toknila_api_types::{ReleaseRun, RunState, RunStep, StepState};
use toknila_core::ReleaseId;
use toknila_core::release_run::ReleaseStep;

use super::codec::Repo;
use super::{StoreError, WriteTx};

/// The entity the entries are journaled under; its id is the milestone's.
pub const ENTITY: &str = "release_run";
/// The most bytes of a step's output kept (the end of it).
const KEEP: usize = 64 * 1024;

/// One journal entry of a release run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    /// A new run with these steps, every one pending.
    RunStarted {
        version: String,
        tag: String,
        steps: Vec<ReleaseStep>,
    },
    /// An interrupted run goes on; the steps that passed stay passed.
    RunResumed,
    /// A step began.
    StepStarted { step: ReleaseStep },
    /// A step passed, with the end of what it printed.
    StepPassed { step: ReleaseStep, output: String },
    /// A step failed; the run stops there.
    StepFailed { step: ReleaseStep, output: String },
    /// Every step passed and the milestone is released.
    RunDone,
    /// The run failed with no step to blame (the milestone could not be saved).
    RunFailed { error: String },
    /// The daemon stopped in the middle of the run, at `step`.
    RunInterrupted { step: Option<ReleaseStep> },
}

impl Entry {
    fn kind(&self) -> &'static str {
        match self {
            Self::RunStarted { .. } => "run_started",
            Self::RunResumed => "run_resumed",
            Self::StepStarted { .. } => "step_started",
            Self::StepPassed { .. } => "step_passed",
            Self::StepFailed { .. } => "step_failed",
            Self::RunDone => "run_done",
            Self::RunFailed { .. } => "run_failed",
            Self::RunInterrupted { .. } => "run_interrupted",
        }
    }
}

/// The end of `text`, at most [`KEEP`] bytes.
pub fn tail(mut text: String) -> String {
    if text.len() > KEEP {
        let mut from = text.len() - KEEP;
        while !text.is_char_boundary(from) {
            from += 1;
        }
        text = text[from..].to_owned();
    }
    text
}

fn last_line(text: &str) -> &str {
    text.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("no output")
}

impl WriteTx<'_> {
    /// Journals one entry of a release run in this transaction.
    pub fn journal_release_run(
        &mut self,
        release: ReleaseId,
        entry: &Entry,
    ) -> Result<(), StoreError> {
        self.append(ENTITY, release, entry.kind(), entry)
    }
}

impl Repo<'_> {
    /// The last release run of a milestone, folded from the journal.
    pub fn release_run(&self, release: ReleaseId) -> Result<Option<ReleaseRun>, StoreError> {
        let mut st = self.conn.prepare(
            "SELECT payload_json FROM events WHERE entity = ?1 AND entity_id = ?2 ORDER BY seq",
        )?;
        let rows = st.query_map(params![ENTITY, release.to_string()], |r| {
            r.get::<_, String>(0)
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(serde_json::from_str::<Entry>(&row?)?);
        }
        Ok(fold(release, &entries))
    }

    /// The milestones that have a release run in the journal.
    pub fn release_run_ids(&self) -> Result<Vec<ReleaseId>, StoreError> {
        let mut st = self
            .conn
            .prepare("SELECT DISTINCT entity_id FROM events WHERE entity = ?1")?;
        let rows = st.query_map([ENTITY], |r| r.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(super::codec::parse(&row?, "release id")?);
        }
        Ok(ids)
    }
}

/// The run the entries tell, or `None` when none of them started one.
pub fn fold(release: ReleaseId, entries: &[Entry]) -> Option<ReleaseRun> {
    let mut run: Option<ReleaseRun> = None;
    for entry in entries {
        match entry {
            Entry::RunStarted {
                version,
                tag,
                steps,
            } => {
                run = Some(ReleaseRun {
                    release: release.to_string(),
                    version: version.clone(),
                    tag: tag.clone(),
                    state: RunState::Running,
                    steps: steps
                        .iter()
                        .map(|s| RunStep {
                            step: *s,
                            label: s.label().to_owned(),
                            state: StepState::Pending,
                            output: String::new(),
                        })
                        .collect(),
                    error: None,
                    resume_from: None,
                });
            }
            other => {
                let Some(r) = run.as_mut() else { continue };
                apply(r, other);
            }
        }
    }
    run
}

fn apply(r: &mut ReleaseRun, entry: &Entry) {
    let set = |r: &mut ReleaseRun, step: ReleaseStep, state: StepState, output: Option<&String>| {
        if let Some(s) = r.steps.iter_mut().find(|s| s.step == step) {
            s.state = state;
            if let Some(out) = output {
                s.output = out.clone();
            }
        }
    };
    match entry {
        Entry::RunStarted { .. } => {}
        Entry::RunResumed => {
            r.state = RunState::Running;
            r.error = None;
            r.resume_from = None;
            for s in &mut r.steps {
                if s.state != StepState::Done {
                    s.state = StepState::Pending;
                }
            }
        }
        Entry::StepStarted { step } => set(r, *step, StepState::Running, None),
        Entry::StepPassed { step, output } => set(r, *step, StepState::Done, Some(output)),
        Entry::StepFailed { step, output } => {
            set(r, *step, StepState::Failed, Some(output));
            r.state = RunState::Failed;
            r.error = Some(format!("{} failed: {}", step.label(), last_line(output)));
        }
        Entry::RunDone => r.state = RunState::Done,
        Entry::RunFailed { error } => {
            r.state = RunState::Failed;
            r.error = Some(error.clone());
        }
        Entry::RunInterrupted { step } => {
            r.state = RunState::Interrupted;
            r.resume_from = *step;
            if let Some(step) = step {
                set(r, *step, StepState::Interrupted, None);
            }
        }
    }
}

/// Where an interrupted run stopped: the step it was in, else the first that had not passed.
pub fn interrupted_at(run: &ReleaseRun) -> Option<ReleaseStep> {
    run.steps
        .iter()
        .find(|s| s.state == StepState::Running)
        .or_else(|| run.steps.iter().find(|s| s.state != StepState::Done))
        .map(|s| s.step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ReleaseStep::*;

    fn started() -> Entry {
        Entry::RunStarted {
            version: "1.0.0".to_owned(),
            tag: "v1.0.0".to_owned(),
            steps: vec![Check, Tag],
        }
    }

    fn states(r: &ReleaseRun) -> Vec<StepState> {
        r.steps.iter().map(|s| s.state).collect()
    }

    #[test]
    fn the_steps_are_folded_from_the_entries() {
        let id = ReleaseId::new();
        let run = fold(
            id,
            &[
                started(),
                Entry::StepStarted { step: Check },
                Entry::StepPassed {
                    step: Check,
                    output: "ok".to_owned(),
                },
                Entry::StepStarted { step: Tag },
            ],
        )
        .expect("a run");
        assert_eq!(run.state, RunState::Running);
        assert_eq!(states(&run), [StepState::Done, StepState::Running]);
        assert_eq!(run.steps[0].output, "ok");
        assert_eq!(fold(id, &[Entry::RunDone]), None, "no run started");
    }

    #[test]
    fn an_interrupted_run_keeps_what_passed_and_resumes_running() {
        let id = ReleaseId::new();
        let mut entries = vec![
            started(),
            Entry::StepPassed {
                step: Check,
                output: String::new(),
            },
            Entry::StepStarted { step: Tag },
        ];
        let run = fold(id, &entries).expect("run");
        assert_eq!(interrupted_at(&run), Some(Tag));
        entries.push(Entry::RunInterrupted { step: Some(Tag) });
        let run = fold(id, &entries).expect("run");
        assert_eq!(run.state, RunState::Interrupted);
        assert_eq!(run.resume_from, Some(Tag));
        assert_eq!(states(&run), [StepState::Done, StepState::Interrupted]);
        entries.push(Entry::RunResumed);
        let run = fold(id, &entries).expect("run");
        assert_eq!(run.state, RunState::Running);
        assert_eq!(states(&run), [StepState::Done, StepState::Pending]);
        assert_eq!(run.resume_from, None);
    }

    #[test]
    fn a_failed_step_names_itself_and_a_new_run_starts_over() {
        let id = ReleaseId::new();
        let mut entries = vec![
            started(),
            Entry::StepFailed {
                step: Check,
                output: "$ x\nboom\n".to_owned(),
            },
        ];
        let run = fold(id, &entries).expect("run");
        assert_eq!(run.state, RunState::Failed);
        assert_eq!(run.error.as_deref(), Some("Check failed: boom"));
        entries.push(started());
        let run = fold(id, &entries).expect("run");
        assert_eq!(run.state, RunState::Running);
        assert_eq!(states(&run), [StepState::Pending, StepState::Pending]);
    }
}
