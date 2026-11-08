import type { ChainDetail } from "../api/types";
import { chainTone } from "../store/groups";
import type { Intent } from "../store/msg";
import { type State, currentProject, runningAgents, slotsOf } from "../store/state";
import { AUTOSTART_HINT, RUN_CHAIN_HINT } from "../store/autostart";
import { Canvas } from "./Canvas";
import { PanelDivider } from "./PanelDivider";
import { Panel } from "./Panel";
import { ThemeButton } from "./ThemeButton";

/// A chain: its top bar (where it stands, a new card, Run chain, the agent slots), the canvas and
/// the panel. The chain's status pill takes the colour of its status: a chain that needs a person
/// reads amber, a running one teal, a finished one green.
export function ChainScreen({ chain, state, dispatch }: { chain: ChainDetail; state: State; dispatch: (i: Intent) => void }) {
  const running = runningAgents(state).length;
  const slots = slotsOf(state);
  const on = chain.chain.autostart;
  return (
    <>
      <header className="top">
        <div className="crumb">
          <b>{currentProject(state)?.key ?? ""}</b><i>/</i><b data-testid="chain-name-top">{chain.chain.name}</b>
        </div>
        <span className={`pill tone-${chainTone(chain.summary.status)}`} data-testid="chain-status" data-status={chain.summary.status}>
          {chain.summary.status.replace(/_/g, " ")}
        </span>
        <span className="muted">{chain.summary.done}/{chain.summary.total} done · {chain.summary.released} released</span>
        {state.error && <span className="error">{state.error}</span>}
        <span className="grow" />
        <span className="pill" data-testid="slots" title="Agents working now / agents allowed at once">
          <span className={`dot${running ? " pulse" : ""}`} />
          {slots ? `${slots.used}/${slots.limit} agents` : `${running} running`}
        </span>
        <label className="autostart" title={AUTOSTART_HINT}>
          <input
            type="checkbox" data-testid="chain-autostart-toggle" checked={on}
            onChange={(e) => dispatch({ type: "setAutostart", chain: chain.chain.id, autostart: e.target.checked })}
          />{" "}
          Autostart
        </label>
        <ThemeButton theme={state.theme} dispatch={dispatch} />
        <button
          type="button" className="btn" data-testid="new-card"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "newCard", chain: chain.chain.id } })}
        >
          New card
        </button>
        <button
          type="button" className={`btn${on ? " on" : " primary"}`} data-testid="run-chain" title={RUN_CHAIN_HINT}
          onClick={() => dispatch({ type: "setAutostart", chain: chain.chain.id, autostart: !on })}
        >
          {on ? "Stop chain" : "Run chain"}
        </button>
      </header>
      <Canvas chain={chain} state={state} dispatch={dispatch} />
      <PanelDivider />
      <Panel state={state} dispatch={dispatch} />
    </>
  );
}
