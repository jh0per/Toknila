// The chain as an epic and a release: the Start node's panel (name and goal), the Finish node's
// panel (the version), and the version a new chain suggests. The daemon decides: its refusal
// shows as it said it.

import type { ChainDetail } from "../api/types";
import type { Effect } from "./msg";
import { withChain } from "./plan";
import { type Route, routePath } from "./route";
import type { State } from "./state";

export type EpicKind = "epic" | "release";

export interface EpicUi {
  /// The open panel: its chain, which node it is for, and the page it was opened on (it hides
  /// when the person goes to another page, such as a card).
  panel: { chain: string; kind: EpicKind; at: string } | undefined;
  /// The version a new chain suggests (the next patch of the newest tag); `undefined` until known.
  suggested: string | undefined;
  /// A save was sent and not yet answered.
  busy: boolean;
  /// The daemon's refusal of the last save.
  error: string | undefined;
}

export const initialEpic: EpicUi = { panel: undefined, suggested: undefined, busy: false, error: undefined };

/// What a save changes; a field left out is kept.
export interface EpicPatch {
  name?: string;
  title?: string;
  goal?: string;
  version?: string;
}

export type EpicIntent =
  | { type: "openEpic"; chain: string; kind: EpicKind }
  | { type: "closeEpic" }
  | { type: "saveEpic"; chain: string; project: string; from: string; patch: EpicPatch }
  | { type: "loadNextVersion"; project: string };

export type EpicResult =
  | { type: "epicSaved"; project: string; from: string; chain: ChainDetail }
  | { type: "epicRefused"; message: string }
  | { type: "nextVersion"; version: string };

export type EpicMsg = EpicIntent | EpicResult;

export type EpicEffect = Extract<EpicIntent, { type: "saveEpic" | "loadNextVersion" }>;

/// The open panel, if it is for the page on screen.
export function epicPanel(s: State): EpicUi["panel"] {
  const p = s.epic.panel;
  return p && p.at === routePath(s.route) && s.chains[p.chain] ? p : undefined;
}

/// The route of a chain after it was renamed.
export const renamedRoute = (project: string, name: string): Route => ({ name: "projectChain", project, chain: name });

/// The reducer of the epic and release panels. A rename moves the page to the chain's new name
/// (`reduce` does the move, then calls `reopen`).
export function reduceEpic(s: State, msg: EpicMsg): [State, Effect[]] {
  const set = (epic: Partial<EpicUi>, base: State = s): State => ({ ...base, epic: { ...base.epic, ...epic } });
  switch (msg.type) {
    case "openEpic":
      return [set({ panel: { chain: msg.chain, kind: msg.kind, at: routePath(s.route) }, error: undefined, busy: false }), []];
    case "closeEpic":
      return [set({ panel: undefined, error: undefined, busy: false }), []];
    case "saveEpic":
      return [set({ busy: true, error: undefined }), [msg]];
    case "epicSaved":
      return [set({ busy: false, error: undefined }, withChain(s, msg.chain)), []];
    case "epicRefused":
      return [set({ busy: false, error: msg.message }), []];
    case "loadNextVersion":
      return [set({ suggested: undefined }), [msg]];
    case "nextVersion":
      return [set({ suggested: msg.version }), []];
  }
}

/// The panel follows the chain to the page of its new name.
export function reopen(s: State): State {
  const p = s.epic.panel;
  return p ? { ...s, epic: { ...s.epic, panel: { ...p, at: routePath(s.route) } } } : s;
}
