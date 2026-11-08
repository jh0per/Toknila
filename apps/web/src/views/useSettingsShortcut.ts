import { useEffect } from "react";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { currentProject } from "../store/state";

/// Ctrl+, (Cmd+, on a Mac) opens the settings from anywhere, on the project in focus.
export function useSettingsShortcut(state: State, dispatch: (i: Intent) => void): void {
  const project = currentProject(state)?.key;
  const inSettings = state.route.name === "settings";
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "," || !(e.metaKey || e.ctrlKey) || e.altKey || e.shiftKey || inSettings) return;
      e.preventDefault();
      dispatch({ type: "navigate", route: { name: "settings", project } });
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dispatch, project, inSettings]);
}
