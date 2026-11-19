//! Parallel agents as use cases: ask core's [`schedule::plan`](toknila_core::schedule::plan) who
//! starts, and start them. The plan is pure and lives in core; this module loads the project's
//! chains and sends each chosen card its start (`Start`, or `ResumeStart` for a story that has
//! begun) through the state machine, inside the command that freed the slot or queued the card.
//! It is the only sender of either but two that start no new process: a simulated agent's own
//! `started` (a fact; an agent that is a process says it only once it runs, and on a Ready card
//! it is a request) and the import of stories already done. The runner itself
//! still starts after the commit, from the effects the start emits ([`crate::external`]);
//! nothing here touches a process.

use std::collections::HashMap;

use toknila_core::graph::ChainView;
use toknila_core::schedule::{Plan, ProjectRun, plan, project_runs, start_event};
use toknila_core::{Project, ProjectId, TicketId};

use crate::chain::run_chain;
use crate::{AppError, AppResult, Reader, UnitOfWork};

/// The views of a project's chains, in chain order.
fn views<R: Reader + ?Sized>(r: &R, project: ProjectId) -> AppResult<Vec<ChainView>> {
    let mut out = Vec::new();
    for chain in r.chains(Some(project))? {
        out.extend(r.chain_view(chain.id)?);
    }
    Ok(out)
}

/// Who starts and who waits in a project, with its current limit. The runners that are alive
/// count (the reader knows them): workers and reviewers each hold a slot.
pub fn plan_of<R: Reader + ?Sized>(r: &R, project: &Project) -> AppResult<Plan> {
    let live = r.live_runners()?;
    Ok(plan(&views(r, project.id)?, project.max_parallel, &live))
}

/// Every project with its plan, in project order.
pub fn plans<R: Reader + ?Sized>(r: &R) -> AppResult<Vec<(Project, Plan)>> {
    r.projects()?
        .into_iter()
        .map(|p| plan_of(r, &p).map(|plan| (p, plan)))
        .collect()
}

/// Every project with its plan and its live runs (its assistant and its cards' runs), in
/// project order.
pub fn agents<R: Reader + ?Sized>(r: &R) -> AppResult<Vec<(Project, Plan, Vec<ProjectRun>)>> {
    let live = r.live_runs()?;
    r.projects()?
        .into_iter()
        .map(|p| {
            let chains = views(r, p.id)?;
            let pairs: Vec<_> = live.iter().map(|(t, role, _)| (*t, *role)).collect();
            let plan = plan(&chains, p.max_parallel, &pairs);
            let runs = project_runs(p.id, &chains, &live);
            Ok((p, plan, runs))
        })
        .collect()
}

/// Place in line of every waiting card, across projects (1 is next).
pub fn queue_positions<R: Reader + ?Sized>(r: &R) -> AppResult<HashMap<TicketId, u32>> {
    Ok(queue(r)?.into_iter().map(|(id, (n, _))| (id, n)).collect())
}

/// Every waiting card, across projects: its place in line (1 is next) and what it says while it
/// waits (core's `schedule::wait_text`).
pub fn queue<R: Reader + ?Sized>(r: &R) -> AppResult<HashMap<TicketId, (u32, String)>> {
    let mut out = HashMap::new();
    for (_, plan) in plans(r)? {
        for (id, text) in plan.queue.iter().zip(&plan.texts) {
            out.extend(plan.position(*id).map(|n| (*id, (n, text.clone()))));
        }
    }
    Ok(out)
}

/// Starts the cards that the plan says have a free slot, each with the event core gives it (a
/// story that has begun goes on, one that has not starts). Called at the end of every chain
/// command, after a runner ended and after a limit change, so a freed slot or a new request is
/// acted on in the same transaction.
pub(crate) fn run<U: UnitOfWork + ?Sized>(uow: &mut U, project: ProjectId) -> AppResult<()> {
    let Some(p) = uow.project(project)? else {
        return Ok(());
    };
    let chains = views(uow, project)?;
    let live = uow.live_runners()?;
    for id in plan(&chains, p.max_parallel, &live).start {
        let Some((chain, event)) = chains.iter().find_map(|c| {
            let t = c.tickets.iter().find(|t| t.id == id)?;
            Some((c.chain.id, start_event(t)))
        }) else {
            continue;
        };
        run_chain(uow, chain, |tx| {
            Ok::<_, AppError>(tx.ticket_event(id, event)?)
        })?;
    }
    Ok(())
}
