//! Turns "a runner's process ended" into a command: one unit of work that journals the exit and
//! moves the ticket through the machine (`toknila_app::runs::ended`).

use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;
use toknila_app::runner::RunEnded;
use toknila_app::{AppError, runs};
use toknila_core::Actor;

use crate::server::now;
use crate::store::Store;

/// Records every process end the runners report, until the sending side is gone.
pub fn supervise(store: Store, mut ended: UnboundedReceiver<RunEnded>) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(e) = ended.recv().await {
            let key = e.run.key.clone();
            let result = store
                .write(Actor::System, now(), move |tx| -> Result<(), AppError> {
                    runs::ended(tx, e)
                })
                .await;
            if let Err(error) = result {
                tracing::error!(ticket = %key, %error, "recording the end of a runner failed");
            }
        }
    })
}
