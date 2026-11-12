//! The roadmap of a project: its chains in order, and moving one of them.

use serde::{Deserialize, Serialize};
use toknila_core::ChainId;
use toknila_core::roadmap::{MoveError, Place, reorder};

use crate::{AppError, AppResult, UnitOfWork, resolve, with_chain};

/// `POST /v1/chains/{chain}/move`: the chain goes right before or right after another chain of
/// the same project; exactly one of the two is given.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct MoveChain {
    /// The chain to go in front of (id or name).
    pub before: Option<String>,
    /// The chain to go behind (id or name).
    pub after: Option<String>,
}

impl From<MoveError> for AppError {
    fn from(e: MoveError) -> Self {
        AppError::bad_request(e.to_string())
    }
}

/// Moves a chain within its project's roadmap, whatever its phase (a finished epic moves too).
/// The chains whose place changed are journaled as updated. Returns the moved chain's id.
pub fn move_chain<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &str,
    project: Option<&str>,
    to: MoveChain,
) -> AppResult<ChainId> {
    let c = resolve::chain_in(uow, project, chain)?;
    let anchor = |name: &str| resolve::chain_in(uow, Some(&c.project_id.to_string()), name);
    let place = match (&to.before, &to.after) {
        (Some(b), None) => Place::Before(anchor(b)?.id),
        (None, Some(a)) => Place::After(anchor(a)?.id),
        _ => {
            return Err(AppError::bad_request(
                "give exactly one of before and after",
            ));
        }
    };
    let chains = uow.chains(Some(c.project_id))?;
    let order: Vec<ChainId> = chains.iter().map(|x| x.id).collect();
    let new = reorder(&order, c.id, place)?;
    for (i, id) in new.iter().enumerate() {
        let position = u32::try_from(i).unwrap_or(u32::MAX);
        let was = chains.iter().find(|x| x.id == *id).map(|x| x.position);
        if was != Some(position) {
            with_chain(uow, *id, |ctx| -> AppResult<()> {
                ctx.edit_chain(|x| x.position = position);
                Ok(())
            })?;
        }
    }
    Ok(c.id)
}
