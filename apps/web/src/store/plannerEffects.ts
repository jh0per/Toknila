// The network side of the planner's Start and Stop. It decides nothing: the daemon's answer goes
// to the reducer.

import { get, post } from "../api/http";
import type { PlannerState } from "../api/types";
import type { Msg } from "./msg";
import type { PlannerEffect } from "./planner";

type Dispatch = (msg: Msg) => void;

/// Runs one effect of the planner.
export function runPlannerEffect(e: PlannerEffect, dispatch: Dispatch): void {
  const base = `/projects/${encodeURIComponent(e.project)}/planner`;
  const request =
    e.call === "load" ? get<PlannerState>(base) : post<PlannerState>(`${base}/${e.call}`, {});
  request
    .then((state) => dispatch({ type: "plannerLoaded", project: e.project, state }))
    .catch((err: unknown) =>
      dispatch({ type: "plannerFailed", error: String(err instanceof Error ? err.message : err) }),
    );
}
