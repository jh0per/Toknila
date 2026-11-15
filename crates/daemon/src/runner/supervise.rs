//! Turns "a runner's process ended" into a command: one unit of work that journals the exit and
//! moves the ticket through the machine (`toknila_app::runs::ended`). A planner's run that ends
//! inside a turn ends the turn too, and the journal says so.

use std::sync::Arc;

use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;
use toknila_app::runner::RunEnded;
use toknila_app::{AppError, runs};
use toknila_core::{Actor, AgentRole, planner};

use super::{Runners, end_planner_turn};
use crate::server::now;
use crate::store::Store;

/// Records every process end the runners report, until the sending side is gone.
pub fn supervise(
    store: Store,
    runners: Arc<Runners>,
    mut ended: UnboundedReceiver<RunEnded>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(e) = ended.recv().await {
            let key = e.run.key.clone();
            if e.run.role == AgentRole::Planner {
                let (project, run) = (planner::project_of(e.run.ticket_id), e.run.id);
                if let Err(error) =
                    end_planner_turn(&store, &runners, project, Some(run), Actor::System).await
                {
                    tracing::error!(planner = %key, %error, "recording the end of a turn failed");
                }
            }
            let result = store
                .write(Actor::System, now(), move |tx| -> Result<bool, AppError> {
                    runs::ended(tx, e)
                })
                .await;
            match result {
                Ok((true, _)) => tracing::info!(
                    ticket = %key,
                    "a worker ended with its story still running and no agent; starting a new run"
                ),
                Ok((false, _)) => {}
                Err(error) => {
                    tracing::error!(ticket = %key, %error, "recording the end of a runner failed");
                }
            }
        }
    })
}
