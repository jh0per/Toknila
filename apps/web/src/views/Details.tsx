import type { Ticket } from "../api/types";
import type { Intent } from "../store/msg";
import { type State, currentChain, currentProject } from "../store/state";
import { ActionBar } from "./ActionBar";
import { CardDesigns } from "./CardDesigns";
import { CardModel } from "./CardModel";
import { CardPlan } from "./CardPlan";
import { Comments } from "./Comments";
import { DependsOn } from "./DependsOn";
import { DetailsMeta, TechnicalDetails } from "./DetailsMeta";
import { ReviewBlock } from "./ReviewBlock";
import { Rounds } from "./Rounds";
import { StatusBlock } from "./StatusBlock";
import { UsageChip } from "./UsageChip";

/// A card's Details: where it stands and the next step first (with the answer box when it asks,
/// the agent's report and the Accept/Return actions when it is in review), then the description,
/// the acceptance criteria and the subtasks, what it depends on, its comments, and, folded away,
/// the technical details.
export function Details({ state, card, dispatch }: { state: State; card: Ticket; dispatch: (i: Intent) => void }) {
  const detail = state.details[card.key];
  const chain = currentChain(state);
  return (
    <div className="details" data-testid="details">
      <div className="details-head">
        <span className="key">{card.key}</span>
        <h3>{card.title}</h3>
        <UsageChip figures={detail?.usage} testId="details-usage" />
      </div>
      <StatusBlock state={state} card={card} detail={detail} dispatch={dispatch} />
      {card.status === "review" && (
        <ReviewBlock detail={detail}>
          <ActionBar key={card.key} card={card} detail={detail} dispatch={dispatch} />
        </ReviewBlock>
      )}
      <DetailsMeta card={card} />
      {detail && (
        <CardModel
          key={`${card.key}:${card.model ?? ""}`} state={state} card={card} canEdit={detail.can_edit}
          projectModel={currentProject(state)?.worker_model ?? undefined} dispatch={dispatch}
        />
      )}
      <CardPlan state={state} card={card} detail={detail} chain={chain} dispatch={dispatch} />
      <CardDesigns state={state} card={card} detail={detail} dispatch={dispatch} />
      <div className="cols">
        <section>
          <h4>Description</h4>
          <pre className="prompt">{card.prompt || "(empty)"}</pre>
        </section>
        <section>
          <h4>Acceptance criteria</h4>
          {card.acceptance.length === 0 ? <p className="muted">None.</p> : (
            <ul className="ck">{card.acceptance.map((a, i) => <li key={i}>{a}</li>)}</ul>
          )}
        </section>
        <section>
          <h4>Subtasks</h4>
          {!detail ? <p className="muted">Loading…</p> : detail.subtasks.length === 0 ? <p className="muted">None.</p> : (
            <ul className="ck">
              {detail.subtasks.map((s) => (
                <li key={s.id} className={s.done ? "d" : ""}>{s.text}</li>
              ))}
            </ul>
          )}
        </section>
      </div>
      <DependsOn state={state} card={card} detail={detail} chain={chain} dispatch={dispatch} />
      <Rounds detail={detail} />
      <Comments key={card.key} card={card} detail={detail} dispatch={dispatch} />
      <TechnicalDetails card={card} ship={detail?.ship} projectDefault={currentProject(state)?.default_profile} />
    </div>
  );
}
