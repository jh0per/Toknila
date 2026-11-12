// Whether Claude Code has asked its first-run question (the trust dialog) in a project's
// workspaces: the daemon journals every `screen` question, and the last one of a project is what
// the project's settings show. Trust is given once per project folder, not per card.

import type { EventRecord } from "../api/types";

/// The last time a card of the project showed a screen question, and what its first line said.
export interface ScreenAsked {
  at: number;
  text: string;
}

/// The path of a project's workspaces folder under the ships root: `<ships>/<project>`, the
/// project key as a path-safe lowercase name (the shipyard's own rule).
export function shipsFolder(ships: string, projectKey: string): string {
  const slug = projectKey.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "x";
  return `${ships.replace(/[\\/]+$/, "")}/${slug}`;
}

/// What the trust note shows for a project: its ships folder once the places are known, and the
/// last question (`null`: none yet). Without a project, neither.
export function trustFacts(
  events: EventRecord[],
  ships: string | undefined,
  projectKey: string | undefined,
): { folder?: string; asked?: ScreenAsked | null } {
  if (!projectKey) return {};
  return {
    folder: ships ? shipsFolder(ships, projectKey) : undefined,
    asked: lastScreenAsked(events, projectKey) ?? null,
  };
}

/// The last `screen` question journaled for a card of `projectKey` (cards are `KEY-n`), if any.
export function lastScreenAsked(events: EventRecord[], projectKey: string): ScreenAsked | undefined {
  for (let i = events.length - 1; i >= 0; i--) {
    const e = events[i];
    const p = (e.payload ?? {}) as Record<string, unknown>;
    if (e.entity !== "ticket" || e.kind !== "question_raised" || p.kind !== "screen") continue;
    if (typeof p.key !== "string" || !p.key.startsWith(`${projectKey}-`)) continue;
    return { at: e.at, text: typeof p.text === "string" && p.text ? p.text : "the agent's screen asks something" };
  }
  return undefined;
}
