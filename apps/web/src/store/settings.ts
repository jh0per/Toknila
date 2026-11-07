// Settings and onboarding: the system checks, the profiles to pick from, adding a project and
// changing its settings. The daemon judges every input; this only keeps what it answered so the
// views can show it next to the field it is about.

import type { CliLink, ListedProfile, PatchProject, Project, RepoCheck, RepoProblem, SystemChecks } from "../api/types";
import {
  type AddForm, type DraftField, type ProjectDraft, applyCheck, editDraft, initialAddForm, mustWait, needsCheck,
  resolvedDraft,
} from "./addForm";
import type { Effect } from "./msg";
import type { State } from "./state";

export type { ProjectDraft } from "./addForm";

export interface SettingsState {
  checks: SystemChecks | undefined;
  /// The `tok` command that ships with the app and whether it is linked into the home folder.
  cli: CliLink | undefined;
  /// Why the last "install the tok command" was refused.
  cliError: string | undefined;
  profiles: ListedProfile[];
  /// Why the last "add project" was refused, per field, and the branches to pick from.
  problems: RepoProblem[];
  branches: string[];
  /// A refusal of the last save or import, with the field it names when it names one.
  error: { field: string | undefined; message: string } | undefined;
  busy: boolean;
  saved: boolean;
  /// Name of the chain the last import made.
  imported: string | undefined;
  /// The add-a-project form, kept here so the folder's suggestion can follow it.
  add: AddForm;
}

export const initialSettings: SettingsState = {
  checks: undefined,
  cli: undefined,
  cliError: undefined,
  profiles: [],
  problems: [],
  branches: [],
  error: undefined,
  busy: false,
  saved: false,
  imported: undefined,
  add: initialAddForm,
};

/// What the person did on these screens.
export type SettingsIntent =
  | { type: "recheck" }
  | { type: "installCli" }
  | { type: "editDraft"; field: DraftField; value: string }
  | { type: "submitProject" }
  | { type: "saveProject"; project: string; patch: PatchProject }
  | { type: "importChain"; project: string; toml: string };

/// What the daemon answered.
export type SettingsResult =
  | { type: "checksLoaded"; checks: SystemChecks }
  | { type: "cliLoaded"; cli: CliLink }
  | { type: "cliRejected"; message: string }
  | { type: "profilesLoaded"; profiles: ListedProfile[] }
  | { type: "repoChecked"; repo_path: string; check: RepoCheck }
  | { type: "addRejected"; problems: RepoProblem[]; branches: string[] }
  | { type: "projectAdded"; project: Project }
  | { type: "projectSaved"; project: Project }
  | { type: "chainImported"; project: string; chain: string }
  | { type: "settingsRejected"; field: string | undefined; message: string };

export type SettingsMsg = SettingsIntent | SettingsResult;

/// Side effects of these screens: the loads, the folder check behind the suggestion, the commands.
export type SettingsEffect =
  | { type: "loadChecks" }
  | { type: "loadProfiles" }
  | { type: "loadCli" }
  | { type: "installCli" }
  | { type: "checkRepo"; repo_path: string; work_branch: string }
  | { type: "addProject"; draft: ProjectDraft }
  | Extract<SettingsIntent, { type: "saveProject" | "importChain" }>;

/// What the screens need fetched when they open.
export const setupLoads: Effect[] = [{ type: "loadChecks" }, { type: "loadProfiles" }];

/// The first problem or refusal about `field`, as text.
export function fieldError(s: SettingsState, field: string): string | undefined {
  return s.problems.find((p) => p.field === field)?.message ?? (s.error?.field === field ? s.error.message : undefined);
}

/// The error that names no field, for a line under the form.
export function formError(s: SettingsState): string | undefined {
  return s.error && !s.error.field ? s.error.message : undefined;
}

export const upsert = (projects: Project[], p: Project): Project[] =>
  projects.some((x) => x.id === p.id) ? projects.map((x) => (x.id === p.id ? p : x)) : [...projects, p];

const start = (s: State): SettingsState => ({
  ...s.settings, problems: [], branches: [], error: undefined, busy: true, saved: false, imported: undefined,
});

const checkOf = (f: AddForm): Effect => ({
  type: "checkRepo", repo_path: f.draft.repo_path.trim(), work_branch: f.draft.work_branch.trim(),
});

const addOf = (f: AddForm): Effect => ({ type: "addProject", draft: resolvedDraft(f) });

/// The reducer of these screens; `projectAdded` also moves the page, which `reduce` does.
export function reduceSettings(s: State, msg: SettingsMsg): [State, Effect[]] {
  const set = (settings: Partial<SettingsState>, projects = s.projects): State => ({
    ...s, projects, settings: { ...s.settings, ...settings },
  });
  const sending = (add: AddForm): State => ({ ...s, settings: { ...start(s), add } });
  switch (msg.type) {
    case "recheck":
      return [set({ checks: undefined }), [{ type: "loadChecks" }]];
    case "editDraft": {
      const add = editDraft(s.settings.add, msg.field, msg.value);
      return [set({ add }), needsCheck(s.settings.add, msg.field, msg.value) ? [checkOf(add)] : []];
    }
    case "repoChecked": {
      const add = applyCheck(s.settings.add, msg.repo_path, msg.check);
      // A form sent before the suggestion came goes out now, with it.
      if (!s.settings.add.waiting || add === s.settings.add) return [set({ add }), []];
      return [sending({ ...add, waiting: false }), [addOf(add)]];
    }
    case "submitProject": {
      const add = s.settings.add;
      if (mustWait(add)) return [sending({ ...add, waiting: true }), [checkOf(add)]];
      return [sending({ ...add, waiting: false }), [addOf(add)]];
    }
    case "saveProject":
    case "importChain":
      return [{ ...s, settings: start(s) }, [msg]];
    case "installCli":
      return [set({ cliError: undefined }), [msg]];
    case "cliLoaded":
      return [set({ cli: msg.cli, cliError: undefined }), []];
    case "cliRejected":
      return [set({ cliError: msg.message }), []];
    case "checksLoaded":
      return [set({ checks: msg.checks }), []];
    case "profilesLoaded":
      return [set({ profiles: msg.profiles }), []];
    case "addRejected": {
      const add = { ...s.settings.add, waiting: false };
      return [set({ problems: msg.problems, branches: msg.branches, busy: false, add }), []];
    }
    case "projectAdded":
      return [set({ busy: false, add: initialAddForm }, upsert(s.projects, msg.project)), [{ type: "loadAgents" }]];
    case "projectSaved":
      return [set({ busy: false, saved: true }, upsert(s.projects, msg.project)), [{ type: "loadAgents" }]];
    case "chainImported":
      return [set({ busy: false, imported: msg.chain }), [{ type: "loadAll" }]];
    case "settingsRejected": {
      const add = { ...s.settings.add, waiting: false };
      return [set({ busy: false, error: { field: msg.field, message: msg.message }, add }), []];
    }
  }
}
