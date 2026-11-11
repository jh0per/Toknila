import type { Comment } from "../api/types";

const CHECK = /^[-*]?\s*(PASS|FAIL|NOT_RUN|BLOCKED)\b[\s:]*(.*)$/;

/// One line of the checks block as a status and its evidence; a line that is no check stays text.
export function parseCheck(line: string): { status: string; text: string } | undefined {
  const m = CHECK.exec(line.trim());
  return m ? { status: m[1], text: m[2] } : undefined;
}

/// Whether the report is a hand-in in the template (the daemon keeps the checks in `how_to_verify`).
export const isHandIn = (c: Comment): boolean => Boolean(c.body.how_to_verify);

/// A hand-in as three blocks: what changed, each check with its status, and what is left. A
/// report that is not a hand-in (an older card) shows its gist and text as before.
export function HandIn({ report }: { report: Comment }) {
  const b = report.body;
  if (!isHandIn(report)) {
    return (
      <div className="report" data-testid="agent-report">
        <p className="report-gist">{b.gist}</p>
        {b.what && <p>{b.what}</p>}
        {b.how_to_verify && <p className="muted">How to check: {b.how_to_verify}</p>}
      </div>
    );
  }
  const checks = (b.how_to_verify ?? "").split("\n").filter((l) => l.trim());
  return (
    <div className="report hand-in" data-testid="agent-report">
      <div data-testid="handin-changed">
        <h5>What changed</h5>
        <p className="report-gist">{b.gist}</p>
        {b.what && <p>{b.what}</p>}
      </div>
      <div data-testid="handin-checks">
        <h5>Checks</h5>
        <ul className="checks">
          {checks.map((l, i) => {
            const c = parseCheck(l);
            return (
              <li key={i} data-status={c?.status}>
                {c ? <><span className="status">{c.status}</span> {c.text}</> : l}
              </li>
            );
          })}
        </ul>
      </div>
      <div data-testid="handin-left">
        <h5>What is left</h5>
        <p>{b.consequences ?? "Nothing was named."}</p>
      </div>
    </div>
  );
}
