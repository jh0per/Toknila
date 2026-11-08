import type { Intent } from "../store/msg";
import { type Tab, visibleTabs } from "../store/route";
import { statusText } from "../store/groups";
import { type State, currentCard, currentProject } from "../store/state";
import { ActionBar } from "./ActionBar";
import { Details } from "./Details";
import { EventsTab } from "./EventsTab";
import { TerminalTab } from "./TerminalTab";
import { ThreadsTab } from "./ThreadsTab";

const LABEL: Record<Tab, string> = {
  details: "Details", events: "Events", chat: "Chat", terminal: "Terminal", threads: "Threads", pr: "Pull request",
};

const PLACEHOLDER: Record<"chat" | "pr", string> = {
  chat: "Talk to this card's agent here. Not available yet.",
  pr: "The pull request of this card, with its checks and review state, will be shown here.",
};

/// The bottom panel for the selected card: tabs, the card's actions once in the header, the tab.
export function Panel({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const card = currentCard(state);
  if (!card) {
    return <section className="panel empty" data-testid="panel"><p className="muted">Select a card to see it here.</p></section>;
  }
  const tabs = visibleTabs(currentProject(state)?.merge.mode === "github", Boolean(card.pr));
  const tab = tabs.includes(state.tab) ? state.tab : "details";
  // In review the actions sit under the agent's report in Details, so the header does not repeat them.
  const actionsInDetails = tab === "details" && card.status === "review";
  return (
    <section className="panel" data-testid="panel">
      <div className="panel-head">
        <div role="tablist" className="tabs">
          {tabs.map((t) => (
            <button
              key={t}
              role="tab"
              aria-selected={tab === t}
              className={tab === t ? "sel" : ""}
              onClick={() => dispatch({ type: "selectTab", tab: t })}
            >
              {LABEL[t]}
            </button>
          ))}
          <span className="who">
            <span className="key">{card.key}</span> {card.title} <span className="muted">{statusText(card.status)}</span>
          </span>
        </div>
        {!actionsInDetails && <ActionBar key={card.key} card={card} detail={state.details[card.key]} dispatch={dispatch} />}
      </div>
      <div role="tabpanel" className="tabpanel" data-tab={tab}>
        {tab === "details" && <Details state={state} card={card} dispatch={dispatch} />}
        {tab === "events" && <EventsTab state={state} card={card} />}
        {tab === "threads" && <ThreadsTab state={state} card={card} dispatch={dispatch} />}
        {tab === "terminal" && <TerminalTab key={card.key} card={card} />}
        {(tab === "chat" || tab === "pr") && <p className="muted" data-testid="placeholder">{PLACEHOLDER[tab]}</p>}
      </div>
    </section>
  );
}
