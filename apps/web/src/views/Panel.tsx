import { type KeyboardEvent, useRef } from "react";
import type { Intent } from "../store/msg";
import { type Tab, visibleTabs } from "../store/route";
import { statusText } from "../store/groups";
import { tabAfterKey } from "../store/shortcuts";
import { type State, currentCard, currentProject } from "../store/state";
import { ActionBar } from "./ActionBar";
import { ChatTab } from "./ChatTab";
import { Details } from "./Details";
import { EventsTab } from "./EventsTab";
import { TerminalTab } from "./TerminalTab";
import { ThreadsTab } from "./ThreadsTab";

const LABEL: Record<Tab, string> = {
  details: "Details", events: "Events", chat: "Chat", terminal: "Terminal", threads: "Threads", pr: "Pull request",
};

const PR_PLACEHOLDER = "The pull request of this card, with its checks and review state, will be shown here.";

/// The bottom panel for the selected card: tabs, the card's actions once in the header, the tab.
/// The arrow keys, Home and End move between the tabs; only the picked tab is in the Tab order.
export function Panel({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const card = currentCard(state);
  const buttons = useRef<Partial<Record<Tab, HTMLButtonElement | null>>>({});
  if (!card) {
    return <section className="panel empty" data-testid="panel"><p className="muted">Select a card to see it here.</p></section>;
  }
  const tabs = visibleTabs(currentProject(state)?.merge.mode === "github", Boolean(card.pr));
  const tab = tabs.includes(state.tab) ? state.tab : "details";
  // In review the actions sit under the agent's report in Details, so the header does not repeat them.
  const actionsInDetails = tab === "details" && card.status === "review";
  const onKeys = (e: KeyboardEvent) => {
    const next = tabAfterKey(tabs, tab, e.key);
    if (!next) return;
    e.preventDefault();
    dispatch({ type: "selectTab", tab: next });
    buttons.current[next]?.focus();
  };
  return (
    <section className="panel" data-testid="panel">
      <div className="panel-head">
        <div role="tablist" aria-label="Card" className="tabs" onKeyDown={onKeys}>
          {tabs.map((t) => (
            <button
              key={t}
              ref={(el) => { buttons.current[t] = el; }}
              role="tab"
              id={`tab-${t}`}
              aria-selected={tab === t}
              aria-controls="card-tabpanel"
              tabIndex={tab === t ? 0 : -1}
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
      <div role="tabpanel" id="card-tabpanel" aria-labelledby={`tab-${tab}`} className="tabpanel" data-tab={tab}>
        {tab === "details" && <Details state={state} card={card} dispatch={dispatch} />}
        {tab === "events" && <EventsTab state={state} card={card} />}
        {tab === "chat" && <ChatTab key={card.key} state={state} card={card} dispatch={dispatch} />}
        {tab === "threads" && <ThreadsTab state={state} card={card} dispatch={dispatch} />}
        {tab === "terminal" && <TerminalTab key={card.key} card={card} />}
        {tab === "pr" && <p className="muted" data-testid="placeholder">{PR_PLACEHOLDER}</p>}
      </div>
    </section>
  );
}
