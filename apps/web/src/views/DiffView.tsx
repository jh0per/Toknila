import { type DiffLine, type Proposal, diffLines, diffText } from "../store/discussion";

const MARK = { same: " ", add: "+", del: "-" } as const;

function Lines({ lines }: { lines: DiffLine[] }) {
  return (
    <pre className="diff-lines">
      {lines.map((l, i) => (
        <span key={i} className={`diff-line ${l.op}`} data-op={l.op}>{`${MARK[l.op]} ${l.text}`}{"\n"}</span>
      ))}
    </pre>
  );
}

/// What the assistant proposes for a card, as a diff of each field it changes: removed lines in
/// red with a minus, added lines in green with a plus, and what stays as it was. A field it does
/// not touch says so.
export function DiffView({ proposal }: { proposal: Proposal }) {
  const { prompt, acceptance } = proposal;
  return (
    <div className="diff" data-testid="edit-diff">
      <section data-field="prompt">
        <h5>Prompt</h5>
        {prompt ? <Lines lines={diffText(prompt.from, prompt.to)} /> : <p className="muted">No change</p>}
      </section>
      <section data-field="acceptance">
        <h5>Acceptance criteria</h5>
        {acceptance ? <Lines lines={diffLines(acceptance.from, acceptance.to)} /> : <p className="muted">No change</p>}
      </section>
    </div>
  );
}
