import { useState } from "react";
import type { ChainDetail, Ticket, TicketDetail } from "../api/types";
import { nodeKey } from "../layout";
import { type EdgeRef, idOf } from "../store/edges";
import type { Intent } from "../store/msg";
import { type State, ticketsInOrder } from "../store/state";

/// What a card waits for, as a list with Remove and an Add form: the keyboard way to edit the
/// dependencies that the diagram edits with the mouse. Core says whether they can still change
/// (`can_move`); the daemon's refusal (a cycle, a card that has started) is shown as it came.
export function DependsOn({ state, card, detail, chain, dispatch }: {
  state: State;
  card: Ticket;
  detail: TicketDetail | undefined;
  chain: ChainDetail | undefined;
  dispatch: (i: Intent) => void;
}) {
  if (!detail || !chain) return null;
  const me = nodeKey({ kind: "ticket", id: card.id });
  const name = (key: string): string => {
    const id = idOf(key);
    const t = chain.tickets.find((x) => x.id === id);
    if (t) return `${t.key} · ${t.title}`;
    return `◆ ${chain.releases.find((r) => r.id === id)?.version ?? id}`;
  };
  const deps = chain.edges.filter((e) => nodeKey(e.to) === me).map((e) => nodeKey(e.from));
  const choices = [
    ...ticketsInOrder(chain).filter((t) => t.id !== card.id).map((t) => nodeKey({ kind: "ticket", id: t.id })),
    ...chain.releases.map((r) => nodeKey({ kind: "release", id: r.id })),
  ].filter((k) => !deps.includes(k));
  const edge = (from: string): EdgeRef => ({ chain: chain.chain.id, from, to: me });
  return (
    <div className="depends" data-testid="depends-on">
      <h4>Depends on</h4>
      {deps.length === 0 ? <p className="muted">Nothing: it can start at once.</p> : (
        <ul>
          {deps.map((k) => (
            <li key={k} data-dep={name(k)}>
              <span>{name(k)}</span>
              {detail.can_move && (
                <button
                  type="button" className="btn mini" data-testid={`dep-remove-${idOf(k)}`} disabled={state.edges.busy}
                  onClick={() => dispatch({ type: "removeEdge", ...edge(k) })}
                >
                  Remove
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
      {detail.can_move ? (
        <AddDependency choices={choices.map((k) => [k, name(k)])} busy={state.edges.busy} onAdd={(from) => dispatch({ type: "addEdge", ...edge(from) })} />
      ) : (
        <p className="muted">What this card waits for can no longer change.</p>
      )}
      {state.edges.error && <p className="error" role="alert" data-testid="dep-error">{state.edges.error}</p>}
    </div>
  );
}

function AddDependency({ choices, busy, onAdd }: { choices: [string, string][]; busy: boolean; onAdd: (from: string) => void }) {
  const [picked, setPicked] = useState("");
  const value = choices.some(([k]) => k === picked) ? picked : (choices[0]?.[0] ?? "");
  if (choices.length === 0) return null;
  return (
    <form
      className="row" data-testid="dep-add-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (value) onAdd(value);
      }}
    >
      <select aria-label="Depends on" data-testid="dep-pick" value={value} onChange={(e) => setPicked(e.target.value)}>
        {choices.map(([k, text]) => <option key={k} value={k}>{text}</option>)}
      </select>
      <button className="btn" type="submit" disabled={busy || !value} data-testid="dep-add">Add dependency</button>
    </form>
  );
}
