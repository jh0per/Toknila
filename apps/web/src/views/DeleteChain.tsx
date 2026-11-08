import type { ChainDeletion } from "../api/types";
import type { Intent } from "../store/msg";
import { type State } from "../store/state";

const cards = (n: number) => `${n} card${n === 1 ? "" : "s"}`;

/// Deleting a chain: what goes is named (the chain and its cards), what stays is said, and the
/// daemon's refusal (an agent works on a card of it) is shown as it came. Afterwards: what
/// became of everything.
export function DeleteChain({ chain: id, state, dispatch }: { chain: string; state: State; dispatch: (i: Intent) => void }) {
  const r = state.removal;
  if (r.chainResult) return <Deleted d={r.chainResult} dispatch={dispatch} />;
  const chain = state.chains[id];
  if (!chain) return null;
  const n = chain.tickets.length;
  return (
    <div className="card-block form" data-testid="delete-chain">
      <h2>Delete chain {chain.chain.name}</h2>
      <p data-testid="delete-chain-what">
        Deletes the chain <b>{chain.chain.name}</b> and its <b>{cards(n)}</b> from Toknila, with their threads, journal
        entries, run files and workspaces. The project folder and the branches Toknila made for the cards stay untouched.
      </p>
      {r.error && (
        <div className="error" role="alert" data-testid="delete-chain-error">
          <p>{r.error.message}</p>
          {r.error.agentsWorking && <p>Stop those agents first, then delete the chain again.</p>}
        </div>
      )}
      <div className="row">
        <button type="button" className="btn danger" disabled={r.busy} data-testid="delete-chain-confirm" onClick={() => dispatch({ type: "deleteChain", chain: id })}>
          Delete chain
        </button>
        <button type="button" className="btn" data-testid="delete-chain-cancel" onClick={() => dispatch({ type: "closeDialog" })}>
          Cancel
        </button>
      </div>
    </div>
  );
}

function Deleted({ d, dispatch }: { d: ChainDeletion; dispatch: (i: Intent) => void }) {
  return (
    <div className="card-block form" data-testid="delete-chain-result">
      <h2>{d.chain.name} was deleted</h2>
      <p>
        {cards(d.tickets)} deleted
        {d.worktrees_removed.length > 0 ? `; workspaces removed: ${d.worktrees_removed.join(", ")}` : ""}.
      </p>
      {d.warnings.length > 0 && (
        <ul className="warnings" data-testid="delete-chain-warnings">
          {d.warnings.map((w) => <li key={w}>{w}</li>)}
        </ul>
      )}
      <button type="button" className="btn" data-testid="delete-chain-done" onClick={() => dispatch({ type: "closeDialog" })}>Close</button>
    </div>
  );
}
