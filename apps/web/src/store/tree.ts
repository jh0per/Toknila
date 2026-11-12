import type { InboxItem, ProjectRun } from "../api/types";
import type { Route } from "./route";
import type { State } from "./state";

const KEY = "toknila.project-open.";

/// What the person left a project in the sidebar: `true` open, `false` closed, `undefined` never
/// touched (closed unless the project is on the screen).
export function readProjectOpen(project: string): boolean | undefined {
  try {
    const v = window.localStorage.getItem(KEY + project);
    return v === "1" ? true : v === "0" ? false : undefined;
  } catch {
    return undefined;
  }
}

/// Remembers, for this viewer, whether the project is open in the sidebar.
export function storeProjectOpen(project: string, open: boolean): void {
  try {
    window.localStorage.setItem(KEY + project, open ? "1" : "0");
  } catch {
    // The node still opens and closes for this visit.
  }
}

/// Whether a project's node shows its rows: always for the project on the screen, else what was
/// remembered, closed when nothing was.
export const projectOpen = (stored: boolean | undefined, onScreen: boolean): boolean => onScreen || stored === true;

/// The items that wait for the person in a project: the item's own project, else its chain's.
export function inboxOf(s: Pick<State, "inbox" | "chains">, project: { id: string }): InboxItem[] {
  const of = (i: InboxItem) => i.project ?? (i.chain ? s.chains[i.chain]?.chain.project_id : undefined);
  return s.inbox.filter((i) => of(i) === project.id);
}

/// How many items wait for the person in a project.
export const waitingIn = (s: Pick<State, "inbox" | "chains">, project: { id: string }): number => inboxOf(s, project).length;

/// What runs in a project as the daemon said: its runs (assistant first, then cards), the slots
/// in use and the limit. Without a report: nothing runs and the limit is the project's own.
export function agentsOf(s: Pick<State, "agents">, project: { id: string; max_parallel?: number }): {
  runs: ProjectRun[];
  used: number;
  limit: number;
} {
  const a = s.agents.find((x) => x.project === project.id);
  return { runs: a?.runs ?? [], used: a?.used ?? 0, limit: a?.limit ?? project.max_parallel ?? 0 };
}

/// The screen of a run of a project: the assistant's page, or the card if the store knows its chain.
export function runRouteIn(s: Pick<State, "chains">, project: { key: string }, run: ProjectRun): Route | undefined {
  if (!run.card) return { name: "planner", project: project.key };
  const id = run.card.id;
  const chain = Object.values(s.chains).find((c) => c.tickets.some((t) => t.id === id));
  return chain && { name: "card", project: project.key, chain: chain.chain.name, key: run.card.key };
}

/// What a key does in the tree, given the row it is pressed on.
export type TreeMove = "next" | "prev" | "first" | "last" | "open" | "close" | "child" | "parent";

/// The move for a key: Up and Down step, Home and End jump, Right opens a closed node (or enters an
/// open one), Left closes an open node (or goes to its parent). `expanded` is the row's
/// `aria-expanded`, absent for a leaf.
export function treeMove(key: string, expanded: boolean | undefined): TreeMove | undefined {
  switch (key) {
    case "ArrowDown": return "next";
    case "ArrowUp": return "prev";
    case "Home": return "first";
    case "End": return "last";
    case "ArrowRight": return expanded === undefined ? undefined : expanded ? "child" : "open";
    case "ArrowLeft": return expanded ? "close" : "parent";
    default: return undefined;
  }
}

/// The index to focus after `move` among `count` visible rows; `undefined` when the move is not
/// about the row list (open, close, child and parent are the tree's own).
export function stepIndex(move: TreeMove, at: number, count: number): number | undefined {
  if (count === 0) return undefined;
  switch (move) {
    case "next": return Math.min(at + 1, count - 1);
    case "prev": return Math.max(at - 1, 0);
    case "first": return 0;
    case "last": return count - 1;
    default: return undefined;
  }
}
