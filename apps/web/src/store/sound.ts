// Sounds for what happens while the window is open. The rule is pure: `soundFor` maps one journal
// event to at most one sound; `hear` keeps what is due for a moment so that a burst plays one,
// the most important. The effect (`playSound`) only plays.

import type { EventRecord, SoundSettings } from "../api/types";
import type { SoundKind } from "../sound/notes";
import type { Effect } from "./msg";
import type { State } from "./state";

/// Events within this long of each other play one sound (the most important).
export const COLLECT_MS = 1500;
/// An event older than this when the window gets it is history (a catch-up after a reconnect).
export const LIVE_MS = 3000;

/// From the most important to the least.
const IMPORTANCE: SoundKind[] = ["problem", "epic_done", "story_done"];

export const allOff: SoundSettings = { story_done: false, epic_done: false, problem: false };

export type SoundIntent = { type: "setSounds"; sounds: SoundSettings } | { type: "playPreview"; kind: SoundKind };
export type SoundResult = { type: "soundsLoaded"; sounds: SoundSettings } | { type: "soundDue" };
export type SoundMsg = SoundIntent | SoundResult;
export type SoundEffect =
  | { type: "saveSounds"; sounds: SoundSettings }
  | { type: "soundWindow" }
  | { type: "playSound"; kind: SoundKind }
  | { type: "previewSound"; kind: SoundKind };

/// What the slice holds: the switches (the daemon's) and the most important sound due.
export interface SoundState {
  switches: SoundSettings;
  due: SoundKind | undefined;
}

export const initialSound: SoundState = { switches: allOff, due: undefined };

type Json = Record<string, unknown> | undefined;
const obj = (v: unknown): Json => (v && typeof v === "object" ? (v as Record<string, unknown>) : undefined);

/// Statuses where a story no longer waits for work: merged, released or cancelled.
const CLOSED = new Set(["merged", "released", "cancelled"]);

/// The latest status of each story of `chain` the window knows: the loaded chain, then every
/// status change of the journal the window has seen (the chain may not be reloaded yet).
function statuses(s: State, chain: string): Map<string, string> {
  const out = new Map<string, string>();
  for (const t of s.chains[chain]?.tickets ?? []) if (t.status !== "proposed") out.set(t.id, t.status);
  for (const e of s.events) {
    if (e.entity !== "ticket" || e.kind !== "status_changed") continue;
    const p = obj(e.payload);
    const t = obj(p?.ticket);
    if (t?.chain_id === chain && typeof p?.to === "string" && typeof t.id === "string" && p.to !== "proposed") out.set(t.id, p.to);
  }
  return out;
}

/// The story that just reached Merged was the last one of its epic that was open.
function lastOpen(s: State, chain: string, ticket: string): boolean {
  const all = statuses(s, chain);
  all.set(ticket, "merged");
  return all.size > 0 && [...all.values()].every((v) => CLOSED.has(v));
}

/// The sound a journal event calls for, if any (one, never several). `s` already holds the event.
export function soundFor(s: State, e: EventRecord): SoundKind | undefined {
  if (e.entity === "release_run") {
    if (e.kind === "step_failed" || e.kind === "run_failed") return "problem";
    return e.kind === "run_done" ? "epic_done" : undefined;
  }
  if (e.entity !== "ticket" || e.kind !== "status_changed") return undefined;
  const p = obj(e.payload);
  const ticket = obj(p?.ticket);
  if (p?.to === "blocked") {
    const reason = obj(p.reason) ?? obj(ticket?.status_reason);
    const kind = obj(reason?.reason)?.kind;
    // The person's own block and the restart that heals itself are silent.
    return kind === "manual" || kind === "runner_lost" ? undefined : "problem";
  }
  if (p?.to !== "merged") return undefined;
  // Work imported as already done (Backlog or Ready straight to Merged) is not a story closing.
  if (p.from === "backlog" || p.from === "ready") return undefined;
  const id = ticket?.id;
  const chain = ticket?.chain_id;
  if (typeof id === "string" && typeof chain === "string" && lastOpen(s, chain, id)) return "epic_done";
  return "story_done";
}

/// A sound for a live event whose switch is on is held for `COLLECT_MS`; the first one starts the
/// wait, a later one in it only replaces a less important one. `received` is when the window got
/// the event (ms since the epoch), as `EventRecord.at`.
export function hear(s: State, e: EventRecord, received: number | undefined): [State, Effect[]] {
  const kind = received === undefined || received - e.at > LIVE_MS ? undefined : soundFor(s, e);
  if (!kind || !s.sound.switches[kind]) return [s, []];
  const due = s.sound.due;
  const best = due && IMPORTANCE.indexOf(due) <= IMPORTANCE.indexOf(kind) ? due : kind;
  return [{ ...s, sound: { ...s.sound, due: best } }, due ? [] : [{ type: "soundWindow" }]];
}

export function reduceSound(s: State, msg: SoundMsg): [State, Effect[]] {
  switch (msg.type) {
    case "soundsLoaded":
      return [{ ...s, sound: { ...s.sound, switches: msg.sounds } }, []];
    case "setSounds":
      return [{ ...s, sound: { ...s.sound, switches: msg.sounds } }, [{ type: "saveSounds", sounds: msg.sounds }]];
    case "playPreview":
      return [s, [{ type: "previewSound", kind: msg.kind }]];
    case "soundDue": {
      const kind = s.sound.due;
      return [{ ...s, sound: { ...s.sound, due: undefined } }, kind ? [{ type: "playSound", kind }] : []];
    }
  }
}
