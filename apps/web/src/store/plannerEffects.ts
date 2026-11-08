// The network side of the planner's Start, Stop and messages. It decides nothing: the daemon's
// answer goes to the reducer.

import { get, getBytes, post, postBytes } from "../api/http";
import type { PlannerState } from "../api/types";
import type { Msg } from "./msg";
import type { PlannerEffect } from "./planner";

type Dispatch = (msg: Msg) => void;

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/// How often a just-started planner's screen is looked at, and how long to wait for it at most.
const SETTLE_POLL_MS = 400;
const SETTLE_MAX_MS = 20_000;

/// Waits until the planner's screen has stopped changing: it has drawn something, and the next
/// look finds the same end. Typing into a program that has not set up its screen yet would lose
/// the text; past the limit the text is typed anyway.
async function settled(base: string): Promise<void> {
  let last = -1;
  for (const until = Date.now() + SETTLE_MAX_MS; Date.now() < until; ) {
    await sleep(SETTLE_POLL_MS);
    const out = await getBytes(`${base}/scrollback`).catch(() => null);
    const end = Number(out?.headers.get("x-toknila-end") ?? 0);
    if (end > 0 && end === last) return;
    last = end;
  }
}

/// Starts the planner (or keeps the live one), waits for its screen when it was just started,
/// types the message and presses Enter in a write of its own, so the line is taken as typed.
async function ask(base: string, e: PlannerEffect, dispatch: Dispatch): Promise<void> {
  const state = await post<PlannerState>(`${base}/start`, { profile: e.profile });
  dispatch({ type: "plannerLoaded", project: e.project, state });
  if (e.wait) await settled(base);
  const typing = new TextEncoder();
  await postBytes(`${base}/input`, typing.encode(e.text ?? ""));
  await sleep(150);
  await postBytes(`${base}/input`, typing.encode("\r"));
  dispatch({ type: "plannerAsked", project: e.project });
}

/// Runs one effect of the planner.
export function runPlannerEffect(e: PlannerEffect, dispatch: Dispatch): void {
  const base = `/projects/${encodeURIComponent(e.project)}/planner`;
  const failed = (err: unknown) =>
    dispatch({ type: "plannerFailed", error: String(err instanceof Error ? err.message : err) });
  if (e.call === "ask") {
    ask(base, e, dispatch).catch(failed);
    return;
  }
  const request =
    e.call === "load" ? get<PlannerState>(base) : post<PlannerState>(`${base}/${e.call}`, e.call === "start" ? { profile: e.profile } : {});
  request.then((state) => dispatch({ type: "plannerLoaded", project: e.project, state })).catch(failed);
}
