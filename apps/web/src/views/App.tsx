import { token } from "../api/http";
import { activeProjects, currentChain, currentProject } from "../store/state";
import { useStore } from "../store/useStore";
import { ChainScreen } from "./ChainScreen";
import { Dialogs } from "./Dialogs";
import { EmptyChains } from "./EmptyChains";
import { Inbox } from "./Inbox";
import { Onboarding } from "./Onboarding";
import { PlannerScreen } from "./PlannerScreen";
import { Settings } from "./Settings";
import { SettingsSidebar } from "./SettingsSidebar";
import { Shortcuts } from "./Shortcuts";
import { Sidebar } from "./Sidebar";
import { UpdateBanner } from "./UpdateBanner";
import { Usage } from "./Usage";
import { useSettingsShortcut } from "./useSettingsShortcut";
import { useShortcutsKey } from "./useShortcutsKey";

export function App() {
  const [state, dispatch] = useStore();
  useSettingsShortcut(state, dispatch);
  useShortcutsKey(state, dispatch);
  if (!token()) {
    return <main><p className="error">Open the address that <code>tok open</code> prints; it carries the access token.</p></main>;
  }
  const r = state.route.name;
  const chain = currentChain(state);
  const project = currentProject(state);
  // The planner's screen has the chain layout (a diagram above, a panel below), chain or not.
  const planning = r === "planner" && Boolean(project) && state.loaded;
  const showChain = (Boolean(chain) || planning) && state.loaded && r !== "inbox" && r !== "settings" && r !== "usage";
  // No project to show yet: the main area is the onboarding, with the add-a-project form on it.
  const onboarding = state.loaded && r !== "settings" && activeProjects(state).length === 0 && !project;
  return (
    <div className="ide">
      {r === "settings" ? <SettingsSidebar state={state} dispatch={dispatch} /> : <Sidebar state={state} dispatch={dispatch} />}
      <main className={showChain ? "has-panel" : "plain"}>
        {state.error && !showChain && <p className="error">{state.error}</p>}
        {!state.loaded ? <p className="muted">Loading…</p> : r === "settings" ? (
          <Settings state={state} dispatch={dispatch} />
        ) : onboarding ? (
          <Onboarding state={state} dispatch={dispatch} />
        ) : r === "usage" ? (
          <Usage state={state} dispatch={dispatch} />
        ) : r === "inbox" ? (
          <Inbox state={state} dispatch={dispatch} />
        ) : planning && project ? (
          <PlannerScreen project={project} state={state} dispatch={dispatch} />
        ) : chain ? (
          <ChainScreen chain={chain} state={state} dispatch={dispatch} />
        ) : project && (r === "project" || r === "chains") ? (
          <EmptyChains project={project} state={state} dispatch={dispatch} />
        ) : (
          <p className="muted empty">
            Pick a chain in the sidebar.
          </p>
        )}
        {/* Last in main, after the top bar the divider measures from, so it reads the panel's height. */}
        <UpdateBanner state={state} dispatch={dispatch} />
      </main>
      <Dialogs state={state} dispatch={dispatch} inline={onboarding} />
      <Shortcuts state={state} dispatch={dispatch} />
    </div>
  );
}
