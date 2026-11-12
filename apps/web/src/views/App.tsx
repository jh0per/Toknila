import type { CSSProperties } from "react";
import { token } from "../api/http";
import { beside } from "../store/assistant";
import { activeProjects, chainsOf, currentChain, currentProject } from "../store/state";
import { useStore } from "../store/useStore";
import { AssistantPanel } from "./AssistantPanel";
import { ChainScreen } from "./ChainScreen";
import { Dialogs } from "./Dialogs";
import { EmptyChains } from "./EmptyChains";
import { Guide } from "./Guide";
import { Inbox } from "./Inbox";
import { Onboarding } from "./Onboarding";
import { OpenedNotice } from "./OpenedNotice";
import { PlannerScreen } from "./PlannerScreen";
import { SwitcherMenu } from "./ProjectSwitcher";
import { Roadmap } from "./Roadmap";
import { Settings } from "./Settings";
import { SettingsSidebar } from "./SettingsSidebar";
import { Shortcuts } from "./Shortcuts";
import { Sidebar } from "./Sidebar";
import { UpdateBanner } from "./UpdateBanner";
import { Usage } from "./Usage";
import { useAssistantShortcut } from "./useAssistantShortcut";
import { useSettingsShortcut } from "./useSettingsShortcut";
import { useShortcutsKey } from "./useShortcutsKey";
import { useSwitcherKey } from "./useSwitcherKey";

export function App() {
  const [state, dispatch] = useStore();
  useSettingsShortcut(state, dispatch);
  useShortcutsKey(state, dispatch);
  useAssistantShortcut(state, dispatch);
  useSwitcherKey(dispatch, state.switcherOpen, Boolean(state.plan.dialog));
  if (!token()) {
    return <main><p className="error">Open the address that <code>tok open</code> prints; it carries the access token.</p></main>;
  }
  const r = state.route.name;
  const chain = currentChain(state);
  const project = currentProject(state);
  // The assistant's own screen is the chat at full width: no diagram, no chain.
  const planning = r === "planner" && Boolean(project) && state.loaded;
  // The project's own page is its roadmap, as long as it has epics; a project without any leads
  // with the question for the assistant.
  const roadmap = r === "project" && Boolean(project) && state.loaded && chainsOf(state, project).length > 0;
  const showChain = !roadmap && !planning && Boolean(chain) && state.loaded && r !== "inbox" && r !== "settings" && r !== "usage" && r !== "guide";
  // No project to show yet: the main area is the onboarding, with the add-a-project form on it.
  const onboarding = state.loaded && r !== "settings" && r !== "guide" && activeProjects(state).length === 0 && !project;
  // The assistant panel sits beside the roadmap, a chain or a card, when it is open.
  const aside = state.assistant.open && state.loaded && !onboarding && beside(state.route) && project !== undefined;
  return (
    <div className={`ide${aside ? " with-assistant" : ""}`} style={aside ? ({ "--assistant-w": `${state.assistant.width}px` } as CSSProperties) : undefined}>
      {r === "settings" ? <SettingsSidebar state={state} dispatch={dispatch} /> : <Sidebar state={state} dispatch={dispatch} />}
      <main className={showChain ? "has-panel" : roadmap ? "roadmap-page" : planning ? "planner-page" : "plain"}>
        {state.error && !showChain && <p className="error">{state.error}</p>}
        {!state.loaded ? <p className="muted">Loading…</p> : r === "settings" ? (
          <Settings state={state} dispatch={dispatch} />
        ) : onboarding ? (
          <Onboarding state={state} dispatch={dispatch} />
        ) : r === "guide" ? (
          <Guide state={state} dispatch={dispatch} />
        ) : r === "usage" ? (
          <Usage state={state} dispatch={dispatch} />
        ) : r === "inbox" ? (
          <Inbox state={state} dispatch={dispatch} />
        ) : roadmap && project ? (
          <Roadmap project={project} state={state} dispatch={dispatch} />
        ) : planning && project ? (
          <PlannerScreen project={project} state={state} dispatch={dispatch} />
        ) : chain ? (
          <ChainScreen chain={chain} state={state} dispatch={dispatch} />
        ) : project && (r === "project" || r === "chains") ? (
          <EmptyChains project={project} state={state} dispatch={dispatch} />
        ) : (
          <p className="muted empty">
            Pick an epic in the sidebar.
          </p>
        )}
        {/* Last in main, after the top bar the divider measures from, so it reads the panel's height. */}
        <UpdateBanner state={state} dispatch={dispatch} />
        <OpenedNotice state={state} dispatch={dispatch} />
      </main>
      {aside && project && <AssistantPanel project={project} state={state} dispatch={dispatch} />}
      <Dialogs state={state} dispatch={dispatch} inline={onboarding} />
      <Shortcuts state={state} dispatch={dispatch} />
      {state.switcherOpen && <SwitcherMenu state={state} dispatch={dispatch} />}
    </div>
  );
}
