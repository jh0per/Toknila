// The network side of the Usage page. It decides nothing: the daemon's answer goes to the reducer.

import { get, put } from "../api/http";
import type { TodayUsage, UsageView } from "../api/types";
import type { Msg } from "./msg";
import type { UsageEffect } from "./usage";

type Dispatch = (msg: Msg) => void;

const failed = (dispatch: Dispatch) => (e: unknown) =>
  dispatch({ type: "usageFailed", error: String(e instanceof Error ? e.message : e) });

/// Runs one effect of the page; the answer is the whole page again.
export function runUsageEffect(e: UsageEffect, dispatch: Dispatch): void {
  // The person's own days: minutes east of UTC, so "today" is the one on their clock.
  const tz = -new Date().getTimezoneOffset();
  if (e.type === "loadToday") {
    // The line is a convenience: an answer that fails leaves the last one on the screen.
    get<TodayUsage>(`/projects/${encodeURIComponent(e.project)}/usage/today?tz=${tz}`)
      .then((view) => dispatch({ type: "todayLoaded", project: e.project, view }))
      .catch(() => {});
    return;
  }
  const load = () => get<UsageView>(`/usage?days=${e.days}&tz=${tz}`);
  const done = (view: UsageView) => dispatch({ type: "usageLoaded", view });
  switch (e.type) {
    case "loadUsage":
      load().then(done).catch(failed(dispatch));
      return;
    case "saveUsageSettings":
      put("/usage/settings", { machine_scan: e.machine_scan })
        .then(load)
        .then(done)
        .catch(failed(dispatch));
      return;
  }
}
