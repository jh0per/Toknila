// Settings and onboarding: the system checks, the profiles to pick from, adding a project and
// changing its settings. The daemon judges every input; this only keeps what it answered so the
// views can show it next to the field it is about.

import type { CliLink, ListedProfile, PatchProject, Project, RepoProblem, SystemChecks } from "../api/types";
import type { Effect } from "./msg";
import type { State } from "./state";

/// What a person types to add a project.
export interface ProjectDraft {
  key: string;
  name: string;
  repo_path: string;
  work_branch: string;
}

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
};

/// What the person did on these screens.
export type SettingsIntent =
  | { type: "recheck" }
  | { type: "installCli" }
  | { type: "addProject"; draft: ProjectDraft }
  | { type: "saveProject"; project: string; patch: PatchProject }
  | { type: "importChain"; project: string; toml: string };

/// What the daemon answered.
export type SettingsResult =
  | { type: "checksLoaded"; checks: SystemChecks }
  | { type: "cliLoaded"; cli: CliLink }
  | { type: "cliRejected"; message: string }
  | { type: "profilesLoaded"; profiles: ListedProfile[] }
  | { type: "addRejected"; problems: RepoProblem[]; branches: string[] }
  | { type: "projectAdded"; project: Project }
  | { type: "projectSaved"; project: Project }
  | { type: "chainImported"; chain: string }
  | { type: "settingsRejected"; field: string | undefined; message: string };

export type SettingsMsg = SettingsIntent | SettingsResult;

/// Side effects of these screens: the loads and the three commands.
export type SettingsEffect =
  | { type: "loadChecks" }
  | { type: "loadProfiles" }
  | { type: "loadCli" }
  | { type: "installCli" }
  | Extract<SettingsIntent, { type: "addProject" | "saveProject" | "importChain" }>;

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

const upsert = (projects: Project[], p: Project): Project[] =>
  projects.some((x) => x.id === p.id) ? projects.map((x) => (x.id === p.id ? p : x)) : [...projects, p];

const start = (s: State): SettingsState => ({
  ...s.settings, problems: [], branches: [], error: undefined, busy: true, saved: false, imported: undefined,
});

/// The reducer of these screens; `projectAdded` also moves the page, which `reduce` does.
export function reduceSettings(s: State, msg: SettingsMsg): [State, Effect[]] {
  const set = (settings: Partial<SettingsState>, projects = s.projects): State => ({
    ...s, projects, settings: { ...s.settings, ...settings },
  });
  switch (msg.type) {
    case "recheck":
      return [set({ checks: undefined }), [{ type: "loadChecks" }]];
    case "addProject":
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
    case "addRejected":
      return [set({ problems: msg.problems, branches: msg.branches, busy: false }), []];
    case "projectAdded":
      return [set({ busy: false }, upsert(s.projects, msg.project)), [{ type: "loadAgents" }]];
    case "projectSaved":
      return [set({ busy: false, saved: true }, upsert(s.projects, msg.project)), [{ type: "loadAgents" }]];
    case "chainImported":
      return [set({ busy: false, imported: msg.chain }), [{ type: "loadAll" }]];
    case "settingsRejected":
      return [set({ busy: false, error: { field: msg.field, message: msg.message } }), []];
  }
}
