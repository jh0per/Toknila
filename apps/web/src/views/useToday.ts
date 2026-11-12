import { useEffect } from "react";
import type { Intent } from "../store/msg";
import { TODAY_POLL_MS } from "../store/spend";
import { type State, currentProject } from "../store/state";
import { agentsOf } from "../store/tree";

/// Keeps today's spend of the open project fresh: asked when the project changes, every minute,
/// and again when the number of running agents changes (a run that ended has just cost something).
export function useToday(state: State, dispatch: (i: Intent) => void): void {
  const project = state.loaded ? currentProject(state) : undefined;
  const key = project?.key;
  const running = project ? agentsOf(state, project).runs.length : 0;
  // biome-ignore lint/correctness/useExhaustiveDependencies: `running` only re-asks.
  useEffect(() => {
    if (!key) return;
    const ask = () => dispatch({ type: "wantToday", project: key });
    ask();
    const id = setInterval(ask, TODAY_POLL_MS);
    return () => clearInterval(id);
  }, [dispatch, key, running]);
}
