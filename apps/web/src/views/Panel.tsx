import type { Intent } from "../store/msg";
import { type Tab, TABS } from "../store/route";
import { statusText } from "../store/groups";
import { type State, currentCard } from "../store/state";
import { ActionBar } from "./ActionBar";
import { Details } from "./Details";
import { EventsTab } from "./EventsTab";
import { TerminalTab } from "./TerminalTab";
import { ThreadsTab } from "./ThreadsTab";

const LABEL: Record<Tab, string> = {
  details: "Details", events: "Events", chat: "Chat", terminal: "Terminal", threads: "Threads", pr: "PR",
};

const PLACEHOLDER: Record<"chat" | "pr", string> = {
  chat: "Talk to this card's agent here. Not available yet.",
  pr: "The pull request of this card, with its checks and review state, will be shown here.",
};

/// The bottom panel for the selected card.
export function Panel({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const card = currentCard(state);
  if (!card) {
    return <section className="panel empty" data-testid="panel"><p className="muted">Select a card to see it here.</p></section>;
  }
  return (
    <section className="panel" data-testid="panel">
      <div role="tablist" className="tabs">
        {TABS.map((t) => (
          <button
            key={t}
            role="tab"
            aria-selected={state.tab === t}
            className={state.tab === t ? "sel" : ""}
            onClick={() => dispatch({ type: "selectTab", tab: t })}
          >
            {LABEL[t]}
          </button>
        ))}
        <span className="who">
          <span className="key">{card.key}</span> {card.title} <span className="muted">{statusText(card.status)}</span>
        </span>
      </div>
      <ActionBar key={card.key} card={card} dispatch={dispatch} />
      <div role="tabpanel" className="tabpanel" data-tab={state.tab}>
        {state.tab === "details" && <Details state={state} card={card} dispatch={dispatch} />}
        {state.tab === "events" && <EventsTab state={state} card={card} />}
        {state.tab === "threads" && <ThreadsTab state={state} card={card} dispatch={dispatch} />}
        {state.tab === "terminal" && <TerminalTab key={card.key} card={card} />}
        {(state.tab === "chat" || state.tab === "pr") && (
          <p className="muted" data-testid="placeholder">{PLACEHOLDER[state.tab]}</p>
        )}
      </div>
    </section>
  );
}
