// The network side of imports and deletion. Called by the effect runner; it decides nothing: the
// daemon's answers and refusals go to the reducer as they are.

import { Refusal, get, post } from "../api/http";
import type { ChainDetail, ImportPathResult, PlanScan, ProjectDeletion } from "../api/types";
import type { ImportEffect, ImportSource } from "./imports";
import type { Msg } from "./msg";
import type { RemovalEffect } from "./removal";

type Dispatch = (msg: Msg) => void;

const enc = encodeURIComponent;
const text = (e: unknown) => (e instanceof Error ? e.message : String(e));

/// Imports the found plans one after another, so their chains and card keys come in list order.
async function importPlans(project: string, paths: string[], dispatch: Dispatch) {
  for (const path of paths) {
    try {
      const result = await post<ImportPathResult>(`/projects/${enc(project)}/import-path`, { path });
      dispatch({ type: "planImported", project, path, result });
    } catch (e) {
      dispatch({ type: "planImported", project, path, message: text(e) });
    }
  }
  dispatch({ type: "plansImported", project });
}

/// Runs one effect of imports or deletion.
export function runImportEffect(e: ImportEffect | RemovalEffect, dispatch: Dispatch): void {
  const refused = (source: ImportSource) => (err: unknown) => dispatch({ type: "importRefused", source, message: text(err) });
  switch (e.type) {
    case "importText":
      post<ChainDetail>(`/projects/${enc(e.project)}/import`, { toml: e.toml })
        .then((chain) => dispatch({ type: "textImported", project: e.project, chain }))
        .catch(refused("paste"));
      return;
    case "importPath":
      post<ImportPathResult>(`/projects/${enc(e.project)}/import-path`, { path: e.path })
        .then((result) => dispatch({ type: "importAnswered", project: e.project, result }))
        .catch(refused("path"));
      return;
    case "loadPlans":
      get<PlanScan>(`/projects/${enc(e.project)}/plans`)
        .then((scan) => dispatch({ type: "plansScanned", project: e.project, reason: e.reason, scan }))
        .catch((err) => dispatch({ type: "scanFailed", project: e.project, message: text(err) }));
      return;
    case "importPlans":
      void importPlans(e.project, e.paths, dispatch);
      return;
    case "deleteProject":
      post<ProjectDeletion>(`/projects/${enc(e.project)}/delete`, { trash_folder: e.trash_folder })
        .then((deletion) => dispatch({ type: "projectDeleted", deletion }))
        .catch((err) =>
          dispatch({
            type: "deleteRefused",
            message: text(err),
            agentsWorking: err instanceof Refusal && err.code === "conflict",
          }));
      return;
  }
}
