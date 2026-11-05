import { token } from "../api/http";
import { currentChain } from "../store/state";
import { useStore } from "../store/useStore";
import { Canvas } from "./Canvas";
import { Inbox } from "./Inbox";
import { Panel } from "./Panel";
import { Sidebar } from "./Sidebar";

export function App() {
  const [state, dispatch] = useStore();
  if (!token()) {
    return <main><p className="error">Open the address that <code>tok open</code> prints; it carries the access token.</p></main>;
  }
  const chain = currentChain(state);
  return (
    <div className="ide">
      <Sidebar state={state} dispatch={dispatch} />
      <main>
        {state.error && <p className="error">{state.error}</p>}
        {!state.loaded ? <p className="muted">Loading…</p> : state.route.name === "inbox" ? (
          <Inbox state={state} dispatch={dispatch} />
        ) : chain ? (
          <>
            <header className="chain-head">
              <h2>{chain.chain.name}</h2>
              <span data-testid="chain-status">{chain.summary.status.replace(/_/g, " ")}</span>
              <span className="muted">{chain.summary.done}/{chain.summary.total} done · {chain.summary.released} released</span>
            </header>
            <Canvas chain={chain} state={state} dispatch={dispatch} />
            <Panel state={state} dispatch={dispatch} />
          </>
        ) : (
          <p className="muted empty">
            {state.projects.length === 0
              ? <>No projects yet. Try <code>tok chain import examples/demo.toml</code>.</>
              : "Pick a chain in the sidebar."}
          </p>
        )}
      </main>
    </div>
  );
}
