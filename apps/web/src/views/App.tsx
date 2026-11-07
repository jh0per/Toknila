import { token } from "../api/http";
import { activeProjects, currentChain, currentProject } from "../store/state";
import { useStore } from "../store/useStore";
import { ChainScreen } from "./ChainScreen";
import { Dialogs } from "./Dialogs";
import { EmptyChains } from "./EmptyChains";
import { Inbox } from "./Inbox";
import { Onboarding } from "./Onboarding";
import { Settings } from "./Settings";
import { Sidebar } from "./Sidebar";
import { Usage } from "./Usage";

export function App() {
  const [state, dispatch] = useStore();
  if (!token()) {
    return <main><p className="error">Open the address that <code>tok open</code> prints; it carries the access token.</p></main>;
  }
  const r = state.route.name;
  const chain = currentChain(state);
  const project = currentProject(state);
  const showChain = Boolean(chain) && state.loaded && r !== "inbox" && r !== "settings" && r !== "usage";
  // No project to show yet: the main area is the onboarding, with the add-a-project form on it.
  const onboarding = state.loaded && r !== "settings" && activeProjects(state).length === 0 && !project;
  return (
    <div className="ide">
      <Sidebar state={state} dispatch={dispatch} />
      <main className={showChain ? "" : "plain"}>
        {state.error && !showChain && <p className="error">{state.error}</p>}
        {!state.loaded ? <p className="muted">Loading…</p> : r === "settings" ? (
          <Settings state={state} dispatch={dispatch} />
        ) : onboarding ? (
          <Onboarding state={state} dispatch={dispatch} />
        ) : r === "usage" ? (
          <Usage state={state} dispatch={dispatch} />
        ) : r === "inbox" ? (
          <Inbox state={state} dispatch={dispatch} />
        ) : chain ? (
          <ChainScreen chain={chain} state={state} dispatch={dispatch} />
        ) : project && (r === "project" || r === "chains") ? (
          <EmptyChains project={project} state={state} dispatch={dispatch} />
        ) : (
          <p className="muted empty">
            Pick a chain in the sidebar.
          </p>
        )}
      </main>
      <Dialogs state={state} dispatch={dispatch} inline={onboarding} />
    </div>
  );
}
