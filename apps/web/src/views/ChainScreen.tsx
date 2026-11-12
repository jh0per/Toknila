import type { ChainDetail } from "../api/types";
import { chainTone } from "../store/groups";
import { versionText } from "../store/phase";
import type { Intent } from "../store/msg";
import { type State, currentProject, slotsOf } from "../store/state";
import { AUTOSTART_HINT, RUN_CHAIN_HINT } from "../store/autostart";
import { AskBox } from "./AskBox";
import { AssistantButton } from "./AssistantButton";
import { viewOf, withView } from "../store/route";
import { Canvas } from "./Canvas";
import { DesignBoard } from "./DesignBoard";
import { NavLink } from "./Link";
import { PhaseChip } from "./PhaseChip";
import { PanelDivider } from "./PanelDivider";
import { SimulatedBanner } from "./SimulatedBanner";
import { Panel } from "./Panel";
import { SpendLine } from "./SpendLine";
import { ThemeButton } from "./ThemeButton";

/// A chain: its top bar (where it stands, a new card, Run chain, the agent slots), the canvas and
/// the panel. The chain's status pill takes the colour of its status: a chain that needs a person
/// reads amber, a running one teal, a finished one green.
export function ChainScreen({ chain, state, dispatch }: { chain: ChainDetail; state: State; dispatch: (i: Intent) => void }) {
  const slots = slotsOf(state);
  const running = slots?.used ?? 0;
  const on = chain.chain.autostart;
  const project = currentProject(state);
  const view = viewOf(state.route);
  return (
    <>
      <header className="top">
        <div className="crumb">
          {project ? (
            <NavLink route={{ name: "project", project: project.key }} dispatch={dispatch} testId="crumb-roadmap" className="crumb-link">
              <b>{project.key}</b>
            </NavLink>
          ) : <b />}
          <i>/</i><b data-testid="chain-name-top">{chain.chain.name}</b>
          {chain.summary.version && <span className="ver" data-testid="chain-version-top">{versionText(chain.summary)}</span>}
        </div>
        <SpendLine state={state} dispatch={dispatch} />
        <nav className="view-switch" aria-label="View" data-testid="view-switch">
          {([undefined, "designs"] as const).map((v) => (
            <NavLink
              key={v ?? "diagram"} route={withView(state.route, v)} dispatch={dispatch} testId={`view-${v ?? "diagram"}`}
              className={`view-tab${view === v ? " on" : ""}`}
            >
              {v ? "Designs" : "Diagram"}
            </NavLink>
          ))}
        </nav>
        <PhaseChip phase={chain.summary.phase} />
        {/* The status pill is only for a chain that needs a person. */}
        {(chain.summary.status === "needs_human" || chain.summary.status === "blocked") && (
          <span className={`pill tone-${chainTone(chain.summary.status)}`} data-testid="chain-status" data-status={chain.summary.status}>
            {chain.summary.status.replace(/_/g, " ")}
          </span>
        )}
        <span className="muted">{chain.summary.done}/{chain.summary.total} done · {chain.summary.released} released</span>
        {state.error && <span className="error">{state.error}</span>}
        {state.notice && <span className="muted" data-testid="answer-notice">{state.notice}</span>}
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
          Start stories automatically
        </label>
        <ThemeButton theme={state.theme} dispatch={dispatch} />
        <AssistantButton state={state} dispatch={dispatch} />
        <button
          type="button" className="btn" data-testid="new-card"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "newCard", chain: chain.chain.id } })}
        >
          New story
        </button>
        <button
          type="button" className={`btn${on ? " on" : " primary"}`} data-testid="run-chain" title={RUN_CHAIN_HINT}
          onClick={() => dispatch({ type: "setAutostart", chain: chain.chain.id, autostart: !on })}
        >
          {on ? "Stop epic" : "Run epic"}
        </button>
        <SimulatedBanner project={project} />
      </header>
      {view === "designs" ? <DesignBoard chain={chain} state={state} dispatch={dispatch} /> : <Canvas
        chain={chain} state={state} dispatch={dispatch}
        onEmpty={project && (
          <div className="empty-state">
            <h2>What should {chain.chain.name} do?</h2>
            <p className="muted">
              This epic has no stories yet. Tell the assistant what it is for and it drafts the stories, or add the first story yourself.
            </p>
            <AskBox
              project={project.key} state={state} dispatch={dispatch} testId="chain-ask" label="What should this epic do?"
              placeholder={`For example: the stories for ${chain.chain.name}`} button="Plan it with the assistant"
            />
            <p className="secondary-links">
              Or{" "}
              <button
                type="button" className="inline-link" data-testid="chain-empty-new-card"
                onClick={() => dispatch({ type: "openDialog", dialog: { kind: "newCard", chain: chain.chain.id } })}
              >
                add a story
              </button>{" "}
              by hand.
            </p>
          </div>
        )}
      />}

      <PanelDivider />
      <Panel state={state} dispatch={dispatch} />
    </>
  );
}
