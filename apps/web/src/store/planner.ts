// A project's planner: whether its process is alive, as the daemon answered. The planner edits the
// plan through its own tools, so the diagram follows the journal like any other change; this
// slice only keeps the Start and Stop state and the refusal of the last command.

import type { PlannerState } from "../api/types";
import type { Effect } from "./msg";
import type { State } from "./state";

/// The built-in profile that launches Claude Code as a planner.
export const DEFAULT_PLANNER_PROFILE = "claude-code-planner";

export interface PlannerSlice {
  /// The daemon's answer per project key.
  byProject: Record<string, PlannerState>;
  /// The refusal of the last command (no repository, no `claude`), shown next to the buttons.
  error: string;
  /// The planner profile Start uses; a person can pick another planner profile.
  profile: string;
}

export const initialPlanner: PlannerSlice = { byProject: {}, error: "", profile: DEFAULT_PLANNER_PROFILE };

/// What the person did.
export type PlannerIntent =
  | { type: "startPlanner"; project: string }
  | { type: "stopPlanner"; project: string }
  /// The planner's terminal says its process ended: ask again whether it runs.
  | { type: "refreshPlanner"; project: string }
  | { type: "pickPlannerProfile"; profile: string };

/// What the daemon answered.
export type PlannerResult =
  | { type: "plannerLoaded"; project: string; state: PlannerState }
  | { type: "plannerFailed"; error: string };

export type PlannerMsg = PlannerIntent | PlannerResult;

export type PlannerEffect = {
  type: "plannerCall";
  project: string;
  call: "load" | "start" | "stop";
  /// The profile a start uses.
  profile?: string;
};

export const loadPlanner = (project: string): Effect => ({ type: "plannerCall", project, call: "load" });

export function reducePlanner(s: State, msg: PlannerMsg): [State, Effect[]] {
  const set = (planner: Partial<PlannerSlice>): State => ({ ...s, planner: { ...s.planner, ...planner } });
  switch (msg.type) {
    case "startPlanner":
      return [
        set({ error: "" }),
        [{ type: "plannerCall", project: msg.project, call: "start", profile: s.planner.profile }],
      ];
    case "stopPlanner":
      return [set({ error: "" }), [{ type: "plannerCall", project: msg.project, call: "stop" }]];
    case "pickPlannerProfile":
      return [set({ profile: msg.profile }), []];
    case "refreshPlanner":
      return [s, [loadPlanner(msg.project)]];
    case "plannerLoaded":
      return [set({ byProject: { ...s.planner.byProject, [msg.project]: msg.state } }), []];
    case "plannerFailed":
      return [set({ error: msg.error }), []];
  }
}

/// The planner of a project as last answered; not running until the daemon says so.
export const plannerOf = (s: State, project: string): PlannerState | undefined => s.planner.byProject[project];
