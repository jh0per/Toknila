import type { Ticket } from "../api/types";
import { type State, ticketEvents } from "../store/state";

/// The card's journal, live from the stream.
export function EventsTab({ state, card }: { state: State; card: Ticket }) {
  const events = ticketEvents(state, card.id);
  if (events.length === 0) return <p className="muted" data-testid="events">No events yet.</p>;
  return (
    <ol className="events" data-testid="events">
      {events.map((e) => (
        <li key={e.seq} data-seq={e.seq}>
          <span className="muted">#{e.seq} {new Date(e.at).toLocaleTimeString()} {e.actor}</span>{" "}
          <b>{e.entity}.{e.kind}</b>
          <code>{JSON.stringify(e.payload)}</code>
        </li>
      ))}
    </ol>
  );
}
