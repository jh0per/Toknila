// Deleting a project from the window, and dropping what the page holds of a deleted project or
// chain (after the command, or when its journal events arrive). The daemon deletes, refuses while
// agents work, and says what became of the folder; this keeps its answer for the dialog.

import type { ChainDeletion, Project, ProjectDeletion } from "../api/types";
import type { Effect } from "./msg";
import { type State, chainOfEvent } from "./state";

export interface RemovalState {
  busy: boolean;
  /// The last refusal, and whether it was because agents still work (409).
  error: { message: string; agentsWorking: boolean } | undefined;
  /// The last request asked for the folder to go to the Trash.
  trashAsked: boolean;
  /// The answer of the last deletion, shown until the dialog closes.
  result: ProjectDeletion | undefined;
  /// The answer of the last chain deletion, shown until the dialog closes.
  chainResult: ChainDeletion | undefined;
}

export const initialRemoval: RemovalState = {
  busy: false, error: undefined, trashAsked: false, result: undefined, chainResult: undefined,
};

export type RemovalIntent =
  | { type: "deleteProject"; project: string; trash_folder: boolean }
  | { type: "deleteChain"; chain: string };

export type RemovalResult =
  | { type: "projectDeleted"; deletion: ProjectDeletion }
  | { type: "chainDeleted"; deletion: ChainDeletion }
  | { type: "deleteRefused"; message: string; agentsWorking: boolean };

export type RemovalMsg = RemovalIntent | RemovalResult;

export type RemovalEffect = RemovalIntent;

/// The folder may go to the Trash only once the person typed the project's key.
export const keyConfirmed = (project: Project, typed: string): boolean => typed.trim() === project.key;

/// What became of the repository folder, in words. A Trash that was asked for and did not take
/// the folder is said plainly.
export function folderText(d: ProjectDeletion, trashAsked: boolean): string {
  const path = d.project.repo_path ?? "";
  switch (d.folder) {
    case "trashed":
      return `The folder ${path} is in the Trash.`;
    case "none":
      return trashAsked ? "There was no folder to move to the Trash." : "The project had no folder.";
    case "kept":
      return trashAsked
        ? `The folder stayed where it was: ${d.folder_error ?? "the Trash did not take it"}`
        : `The folder ${path} stays where it is, with its ship/* branches.`;
  }
}

/// The store without a chain: its read model, its cards' details and threads, and the journal
/// entries the page placed on it.
export function dropChain(s: State, chainId: string): State {
  const chain = s.chains[chainId];
  if (!chain) return s;
  const keys = new Set(chain.tickets.map((t) => t.key));
  const keep = <T>(r: Record<string, T>) => Object.fromEntries(Object.entries(r).filter(([k]) => !keys.has(k)));
  const { [chainId]: _gone, ...chains } = s.chains;
  return {
    ...s,
    chains,
    chainOrder: s.chainOrder.filter((id) => id !== chainId),
    details: keep(s.details),
    threads: keep(s.threads),
    events: s.events.filter((e) => chainOfEvent(s, e) !== chainId),
  };
}

/// The store without a project and its chains.
export function dropProject(s: State, projectId: string): State {
  const mine = s.chainOrder.filter((id) => s.chains[id]?.chain.project_id === projectId);
  const without = mine.reduce(dropChain, s);
  return {
    ...without,
    projects: without.projects.filter((p) => p.id !== projectId),
    agents: without.agents.filter((a) => a.project !== projectId),
    events: without.events.filter((e) => !(e.entity === "project" && e.entity_id === projectId)),
  };
}

/// Whether the route names a project or a chain the store no longer has.
export function routeGone(s: State): boolean {
  const r = s.route;
  if ("project" in r && r.project && !s.projects.some((p) => p.key === r.project || p.id === r.project)) return true;
  if (r.name === "projectChain" || r.name === "card" || r.name === "chain") {
    return !Object.values(s.chains).some((c) => c.chain.name === r.chain);
  }
  return false;
}

/// The reducer of deletion; `reduce` sends the page home afterwards.
export function reduceRemoval(s: State, msg: RemovalMsg): [State, Effect[]] {
  const set = (removal: Partial<RemovalState>, base: State = s): State => ({ ...base, removal: { ...base.removal, ...removal } });
  switch (msg.type) {
    case "deleteProject":
      return [set({ busy: true, error: undefined, result: undefined, trashAsked: msg.trash_folder }), [msg]];
    case "deleteChain":
      return [set({ busy: true, error: undefined, chainResult: undefined }), [msg]];
    case "chainDeleted":
      return [
        set({ busy: false, chainResult: msg.deletion }, dropChain(s, msg.deletion.chain.id)),
        [{ type: "loadInbox" }, { type: "loadAgents" }],
      ];
    case "projectDeleted":
      return [
        set({ busy: false, result: msg.deletion }, dropProject(s, msg.deletion.project.id)),
        [{ type: "loadInbox" }, { type: "loadAgents" }],
      ];
    case "deleteRefused":
      return [set({ busy: false, error: { message: msg.message, agentsWorking: msg.agentsWorking } }), []];
  }
}
