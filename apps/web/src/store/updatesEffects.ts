// The shell side of updates: Tauri IPC to the desktop app. Absent in a browser, where every
// effect does nothing. It decides nothing; the shell's answers go to the reducer as they are.

import type { Msg } from "./msg";
import { shellInvoke } from "./shell";
import type { RestartAnswer, UpdateEffect, UpdateView } from "./updates";

type Dispatch = (msg: Msg) => void;

/// How often the page asks the shell for its state (a local call, not the network).
const POLL_MS = 15_000;

const message = (e: unknown) => String(e instanceof Error ? e.message : e);

/// Runs one effect; `watchUpdates` returns how to stop the polling it starts.
export function runUpdateEffect(e: UpdateEffect, dispatch: Dispatch): (() => void) | undefined {
  const call = shellInvoke();
  if (!call) return undefined;
  const view = (v: unknown) => dispatch({ type: "updateViewLoaded", view: v as UpdateView });
  const failed = (err: unknown) => dispatch({ type: "updateFailed", message: message(err) });
  switch (e.type) {
    case "watchUpdates": {
      const poll = () => void call("update_status").then(view).catch(() => undefined);
      poll();
      const t = window.setInterval(poll, POLL_MS);
      return () => window.clearInterval(t);
    }
    case "checkForUpdates":
      call("check_for_updates").then(view).catch(failed);
      return undefined;
    case "restartToUpdate":
      call("restart_to_update", { confirmed: e.confirmed })
        .then((a) => {
          const answer = a as RestartAnswer;
          if (answer.outcome === "warn") dispatch({ type: "updateWarned", message: answer.message });
        })
        .catch(failed);
      return undefined;
  }
}
