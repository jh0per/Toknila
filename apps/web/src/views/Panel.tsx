import type { Intent } from "../store/msg";
import { type Tab, TABS } from "../store/route";
import { type State, currentCard } from "../store/state";
import { Details } from "./Details";
import { EventsTab } from "./EventsTab";

const LABEL: Record<Tab, string> = {
  details: "Details", events: "Events", chat: "Chat", terminal: "Terminal", threads: "Threads", pr: "PR",
};

const PLACEHOLDER: Record<"chat" | "terminal" | "threads" | "pr", string> = {
  chat: "Talk to this card's agent here. Not available yet.",
  terminal: "The agent's terminal will be shown here once ships run real agents.",
  threads: "Side threads of this card will be listed here. Not available yet.",
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
      </div>
      <div role="tabpanel" className="tabpanel" data-tab={state.tab}>
        {state.tab === "details" && <Details state={state} card={card} />}
        {state.tab === "events" && <EventsTab state={state} card={card} />}
        {state.tab !== "details" && state.tab !== "events" && (
          <p className="muted" data-testid="placeholder">{PLACEHOLDER[state.tab]}</p>
        )}
      </div>
    </section>
  );
}
