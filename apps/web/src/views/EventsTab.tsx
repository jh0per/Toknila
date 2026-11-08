import type { Ticket } from "../api/types";
import { whenText } from "../store/chat";
import { WHO, eventWords } from "../store/eventWords";
import { type State, ticketEvents } from "../store/state";

/// The card's journal, live from the stream: one line in words per event, who and when, with the
/// raw event behind a disclosure for whoever needs it.
export function EventsTab({ state, card }: { state: State; card: Ticket }) {
  const events = ticketEvents(state, card.id);
  if (events.length === 0) return <p className="muted" data-testid="events">Nothing has happened to this card yet.</p>;
  return (
    <ol className="events" data-testid="events">
      {events.map((e) => (
        <li key={e.seq} data-seq={e.seq} data-event={`${e.entity}.${e.kind}`}>
          <span className="when muted">{whenText(e.at)} · {WHO[e.actor]}</span>
          <span className="words" data-testid="event-words">{eventWords(e)}</span>
          <details className="raw" data-testid="event-raw">
            <summary>Raw event</summary>
            <code>#{e.seq} {e.entity}.{e.kind}</code>
            <pre>{JSON.stringify(e.payload, null, 2)}</pre>
          </details>
        </li>
      ))}
    </ol>
  );
}
