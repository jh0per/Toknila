// The network side of settings and onboarding. Called by the effect runner; it decides nothing:
// every refusal comes from the daemon and is handed to the reducer as it is.

import { Refusal, get, patch, post } from "../api/http";
import type { ChainDetail, ProfileList, Project, RepoCheck, SystemChecks } from "../api/types";
import type { Msg } from "./msg";
import type { SettingsEffect } from "./settings";

type Dispatch = (msg: Msg) => void;

const reject = (dispatch: Dispatch, e: unknown, fallback?: string) => {
  const r = e instanceof Refusal ? e : undefined;
  const message = e instanceof Error ? e.message : String(e);
  dispatch({ type: "settingsRejected", field: r?.field ?? fallback, message });
};

async function addProject(e: Extract<SettingsEffect, { type: "addProject" }>, dispatch: Dispatch) {
  const { key, name, repo_path, work_branch } = e.draft;
  const check = await post<RepoCheck>("/repos/check", { repo_path, work_branch });
  if (check.problems.length) {
    dispatch({ type: "addRejected", problems: check.problems, branches: check.branches });
    return;
  }
  const project = await post<Project>("/projects", {
    key, name: name.trim() || undefined, repo_path, work_branch,
  });
  dispatch({ type: "projectAdded", project });
}

/// Runs one effect of these screens; failures become a `settingsRejected` message.
export function runSettingsEffect(e: SettingsEffect, dispatch: Dispatch): void {
  switch (e.type) {
    case "loadChecks":
      get<SystemChecks>("/system/checks")
        .then((checks) => dispatch({ type: "checksLoaded", checks }))
        .catch((err) => reject(dispatch, err));
      return;
    case "loadProfiles":
      get<ProfileList>("/profiles")
        .then((l) => dispatch({ type: "profilesLoaded", profiles: l.profiles }))
        .catch((err) => reject(dispatch, err));
      return;
    case "addProject":
      // A refusal of the create itself (key in use, malformed key) is about the key.
      addProject(e, dispatch).catch((err) => reject(dispatch, err, "key"));
      return;
    case "saveProject":
      patch<Project>(`/projects/${encodeURIComponent(e.project)}`, e.patch)
        .then((project) => dispatch({ type: "projectSaved", project }))
        .catch((err) => reject(dispatch, err));
      return;
    case "importChain":
      post<ChainDetail>(`/projects/${encodeURIComponent(e.project)}/import`, { toml: e.toml })
        .then((d) => dispatch({ type: "chainImported", chain: d.chain.name }))
        .catch((err) => reject(dispatch, err));
      return;
  }
}
