import type { ChainDetail } from "../api/types";
import type { Intent } from "../store/msg";
import { type State, currentProject, runningAgents, slotsOf } from "../store/state";
import { AUTOSTART_HINT, RUN_CHAIN_HINT } from "../store/autostart";
import { Canvas } from "./Canvas";
import { PanelDivider } from "./PanelDivider";
import { Panel } from "./Panel";

/// A chain: its top bar (where it stands, a new card, the agent slots), the canvas and the panel.
export function ChainScreen({ chain, state, dispatch }: { chain: ChainDetail; state: State; dispatch: (i: Intent) => void }) {
  const running = runningAgents(state).length;
  const slots = slotsOf(state);
  return (
    <>
      <header className="top">
        <div className="crumb">
          <b>{currentProject(state)?.key ?? ""}</b><i>/</i><b data-testid="chain-name-top">{chain.chain.name}</b>
        </div>
        <span className="pill" data-testid="chain-status">{chain.summary.status.replace(/_/g, " ")}</span>
        <span className="muted">{chain.summary.done}/{chain.summary.total} done · {chain.summary.released} released</span>
        <button
          type="button" className="btn" data-testid="new-card"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "newCard", chain: chain.chain.id } })}
        >
          New card
        </button>
        <button
          type="button" className={`btn${chain.chain.autostart ? " on" : ""}`} data-testid="run-chain" title={RUN_CHAIN_HINT}
          onClick={() => dispatch({ type: "setAutostart", chain: chain.chain.id, autostart: !chain.chain.autostart })}
        >
          {chain.chain.autostart ? "Stop chain" : "Run chain"}
        </button>
        <label className="autostart" title={AUTOSTART_HINT}>
          <input
            type="checkbox" data-testid="chain-autostart-toggle" checked={chain.chain.autostart}
            onChange={(e) => dispatch({ type: "setAutostart", chain: chain.chain.id, autostart: e.target.checked })}
          />{" "}
          Autostart
        </label>
        {state.error && <span className="error">{state.error}</span>}
        <span className="grow" />
        <span className="pill" data-testid="slots" title="Agents working now / agents allowed at once">
          <span className={`dot${running ? " pulse" : ""}`} />
          {slots ? `${slots.used}/${slots.limit} agents` : `${running} running`}
        </span>
      </header>
      <Canvas chain={chain} state={state} dispatch={dispatch} />
      <PanelDivider />
      <Panel state={state} dispatch={dispatch} />
    </>
  );
}
