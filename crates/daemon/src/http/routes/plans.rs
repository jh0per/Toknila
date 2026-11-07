//! `/projects/{project}/import-path` and `/projects/{project}/plans`: plans the daemon reads from
//! disk. Reading and walking are blocking jobs run outside any transaction; each chain then goes
//! through the ordinary chain import in its own transaction.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use toknila_app::{AppError, chains, resolve};
use toknila_core::Actor;
use toknila_core::plans::PlanScan;

use super::{ApiResult, load_chain_detail, read, stamp};
use crate::http::dto::{ChainDetail, FolderNote, ImportPath, ImportPathResult, ImportedChain};
use crate::http::error::{ApiError, ErrorBody};
use crate::plan_files::{self, Candidate, Found};
use crate::server::AppState;

/// Runs blocking filesystem work off the async workers.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> ApiResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ApiError(AppError::internal(format!("reading the disk stopped: {e}"))))
}

#[utoipa::path(post, path = "/projects/{project}/import-path", tag = "chains", request_body = ImportPath,
    params(("project" = String, Path, description = "Project id or key")),
    responses(
        (status = 201, body = ImportPathResult, description = "At least one chain was imported"),
        (status = 200, body = ImportPathResult, description = "A folder of spec folders none of which could be imported; see `failed`"),
        (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Reads a path on this machine and imports what it holds through the ordinary chain import. A
/// TOML chain file or a spec folder makes one chain, or fails as a whole with a message that
/// names the path (and, for a `tasks.md`, the line). A folder of spec folders makes one chain per
/// spec folder, each on its own, and lists the subfolders it skipped and those that failed. Each
/// chain keeps the path it was read from as its `source`.
pub async fn import_path(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(input): Json<ImportPath>,
) -> ApiResult<(StatusCode, Json<ImportPathResult>)> {
    let key = project.clone();
    read(&state, move |repo| Ok(resolve::project(repo, &key)?)).await?;
    let found = blocking(move || plan_files::read(&input.path)).await??;
    let mut out = ImportPathResult::default();
    match found {
        Found::One(c) => out.chains.push(import(&state, &project, c).await?),
        Found::Many {
            spec_folders,
            skipped,
        } => {
            out.skipped = skipped;
            for c in spec_folders {
                let (folder, path) = (c.folder.clone(), c.path.clone());
                match import(&state, &project, c).await {
                    Ok(chain) => out.chains.push(chain),
                    Err(e) => out.failed.push(FolderNote {
                        folder,
                        path,
                        reason: e.0.message,
                    }),
                }
            }
        }
    }
    let status = if out.chains.is_empty() {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(out)))
}

/// Imports one chain file or spec folder as its own command. A refusal of the import names the
/// path; a file that could not be read already does.
async fn import(state: &AppState, project: &str, c: Candidate) -> ApiResult<ImportedChain> {
    let prepared = c.chain.map_err(ApiError::bad_request)?;
    let (at, prices) = stamp(state);
    let (project, source, toml) = (project.to_owned(), c.path.clone(), prepared.toml);
    let path = c.path.clone();
    let (chain, _) = state
        .store
        .write(Actor::Human, at, move |tx| -> ApiResult<ChainDetail> {
            let id = chains::import(tx, &project, &toml, Some(source))?;
            load_chain_detail(&tx.repo(), id, at, &prices)
        })
        .await
        .map_err(|e| ApiError(AppError::new(e.0.kind, format!("{path}: {}", e.0.message))))?;
    Ok(ImportedChain {
        folder: c.folder,
        path: c.path,
        cards: prepared.counts.cards,
        tasks: prepared.counts.tasks,
        done_cards: prepared.counts.done_cards,
        out_of_order: prepared.out_of_order,
        chain,
    })
}

#[utoipa::path(get, path = "/projects/{project}/plans", tag = "chains",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = PlanScan), (status = 404, body = ErrorBody)))]
/// Lists the plans in the project's repository: `specs/*/tasks.md` and `plans/*.toml`, one level
/// deep and never the whole tree, each with its card count and whether a chain of the project
/// was imported from it, and whether the repository has a `.specify/` folder. A project without a
/// repository, or whose folder is gone, has none. Importing one is `import-path` with its `path`.
pub async fn list_plans(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<PlanScan>> {
    let (repo, sources) = read(&state, move |r| {
        let p = resolve::project(r, &project)?;
        let chains = r.chains(Some(p.id))?;
        Ok((
            p.repo_path,
            chains
                .into_iter()
                .filter_map(|c| c.source)
                .collect::<Vec<_>>(),
        ))
    })
    .await?;
    Ok(Json(
        blocking(move || plan_files::scan(repo.as_deref(), &sources)).await?,
    ))
}
