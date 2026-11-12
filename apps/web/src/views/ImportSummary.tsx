import type { ImportPathResult, ImportedChain } from "../api/types";
import { epicTitle } from "../store/title";

const counts = (c: ImportedChain): string =>
  [
    `${c.cards} ${c.cards === 1 ? "story" : "stories"}`,
    c.tasks ? `${c.tasks} task${c.tasks === 1 ? "" : "s"}` : "",
    c.done_cards ? `${c.done_cards} already done` : "",
  ].filter(Boolean).join(", ");

/// What an import made: one line per chain, then what was skipped and what failed.
export function ImportSummary({ result }: { result: ImportPathResult }) {
  return (
    <div className="import-result" data-testid="import-result">
      {result.chains.map((c) => (
        <div key={c.chain.chain.id} data-chain={c.chain.chain.name}>
          <p>Imported <b>{epicTitle(c.chain)}</b> from {c.folder}: {counts(c)}.</p>
          {c.out_of_order.map((line) => <p key={line} className="muted">{line}</p>)}
        </div>
      ))}
      {result.skipped.map((f) => <p key={f.path} className="muted">Skipped {f.folder}: {f.reason}</p>)}
      {result.failed.map((f) => <p key={f.path} className="error">Not imported {f.folder}: {f.reason}</p>)}
      {result.chains.length === 0 && result.failed.length === 0 && <p className="muted">Nothing was imported.</p>}
    </div>
  );
}
