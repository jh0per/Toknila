// The network side of settings and onboarding. Called by the effect runner; it decides nothing:
// every refusal comes from the daemon and is handed to the reducer as it is.

import { Refusal, get, patch, post } from "../api/http";
import type { ChainDetail, CliLink, Places, ProfileList, Project, RepoCheck, SystemChecks } from "../api/types";
import { newProjectBody } from "./addForm";
import type { Msg } from "./msg";
import { pickFolder } from "./shell";
import type { SettingsEffect } from "./settings";

type Dispatch = (msg: Msg) => void;

const reject = (dispatch: Dispatch, e: unknown, fallback?: string) => {
  const r = e instanceof Refusal ? e : undefined;
  const message = e instanceof Error ? e.message : String(e);
  dispatch({ type: "settingsRejected", field: r?.field ?? fallback, message });
};

async function addProject(e: Extract<SettingsEffect, { type: "addProject" }>, dispatch: Dispatch) {
  const { repo_path, work_branch } = e.draft;
  const check = await post<RepoCheck>("/repos/check", { repo_path, work_branch });
  if (check.problems.length) {
    dispatch({ type: "addRejected", problems: check.problems, branches: check.branches });
    return;
  }
  const project = await post<Project>("/projects", newProjectBody(e.draft));
  dispatch({ type: "projectAdded", project });
}

/// Runs one effect of these screens; failures become a `settingsRejected` message.
export function runSettingsEffect(e: SettingsEffect, dispatch: Dispatch): void {
  switch (e.type) {
    case "checkRepo":
      // Only for the suggestion; a failure here shows when the form is sent.
      post<RepoCheck>("/repos/check", { repo_path: e.repo_path, work_branch: e.work_branch })
        .then((check) => dispatch({ type: "repoChecked", repo_path: e.repo_path, check }))
        .catch(() => undefined);
      return;
    case "prepareRepo":
      // The daemon's own sentence is the refusal; it is shown next to the folder.
      post<RepoCheck>("/repos/prepare", { repo_path: e.repo_path, fix: e.fix })
        .then((check) => dispatch({ type: "repoPrepared", repo_path: e.repo_path, check }))
        .catch((err) => dispatch({
          type: "fixRefused", repo_path: e.repo_path, message: err instanceof Error ? err.message : String(err),
        }));
      return;
    case "pickFolder":
      pickFolder()
        .then((path) => path && dispatch({ type: "editDraft", field: "repo_path", value: path, now: true }))
        .catch(() => undefined);
      return;
    case "loadChecks":
      get<SystemChecks>("/system/checks")
        .then((checks) => dispatch({ type: "checksLoaded", checks }))
        .catch((err) => reject(dispatch, err));
      return;
    case "loadCli":
      get<CliLink>("/system/cli")
        .then((cli) => dispatch({ type: "cliLoaded", cli }))
        .catch(() => undefined);
      return;
    case "installCli":
      post<CliLink>("/system/cli/install", {})
        .then((cli) => dispatch({ type: "cliLoaded", cli }))
        .catch((err) => dispatch({ type: "cliRejected", message: err instanceof Error ? err.message : String(err) }));
      return;
    case "loadProfiles":
      get<ProfileList>("/profiles")
        .then((l) => dispatch({ type: "profilesLoaded", profiles: l.profiles, folder: l.folder }))
        .catch((err) => reject(dispatch, err));
      return;
    case "loadPlaces":
      get<Places>("/system/places")
        .then((places) => dispatch({ type: "placesLoaded", places }))
        .catch(() => undefined);
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
        .then((d) => dispatch({ type: "chainImported", project: e.project, chain: d.chain.name }))
        .catch((err) => reject(dispatch, err));
      return;
  }
}
