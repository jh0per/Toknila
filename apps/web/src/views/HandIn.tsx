import type { Comment, InboxHandIn, SinceReturn } from "../api/types";

const CHECK = /^[-*]?\s*(PASS|FAIL|NOT_RUN|BLOCKED)\b[\s:]*(.*)$/;

/// One line of the checks block as a status and its evidence; a line that is no check stays text.
export function parseCheck(line: string): { status: string; text: string } | undefined {
  const m = CHECK.exec(line.trim());
  return m ? { status: m[1], text: m[2] } : undefined;
}

/// Whether the report is a hand-in in the template (the daemon keeps the checks in `how_to_verify`).
export const isHandIn = (c: Comment): boolean => Boolean(c.body.how_to_verify);

/// What changed since the Return this hand-in answers: the commits and the changed files, as the
/// daemon derived them from the returned HEAD to the new one.
export function SinceReturnBlock({ since }: { since: SinceReturn }) {
  const short = (c: string) => c.slice(0, 7);
  return (
    <div className="since-return" data-testid="handin-since">
      <h5>Since the Return</h5>
      <p className="muted">
        Round {since.round}, {short(since.from)}..{short(since.to)}
      </p>
      {since.commits.length === 0 ? (
        <p>No commits.</p>
      ) : (
        <ul className="commits">
          {since.commits.map((c, i) => (
            <li key={i}>{c}</li>
          ))}
          {Boolean(since.more_commits) && <li className="muted">and {since.more_commits} more commits</li>}
        </ul>
      )}
      {since.files.length > 0 && (
        <p className="files">
          Changed files: {since.files.join(", ")}
          {Boolean(since.more_files) && ` and ${since.more_files} more`}
        </p>
      )}
    </div>
  );
}

/// A hand-in as three blocks: what changed, each check with its status, and what is left. A
/// report that is not a hand-in (an older card) shows its gist and text as before.
export function HandIn({ report, since }: { report: Comment; since?: SinceReturn | null }) {
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
  const checks = (b.how_to_verify ?? "")
    .split("\n")
    .filter((l) => l.trim())
    .map((l) => {
      const c = parseCheck(l);
      return c ? { status: c.status, text: c.text } : { text: l };
    });
  return <HandInBlocks gist={b.gist} what={b.what} checks={checks} left={b.consequences} since={since} />;
}

/// The hand-in the inbox item carries (core's `InboxHandIn`): the same three blocks, no request.
export function InboxHandInBlocks({ handIn }: { handIn: InboxHandIn }) {
  return <HandInBlocks gist={handIn.gist} what={handIn.what} checks={handIn.checks} left={handIn.left} since={handIn.since_return} />;
}

type Parts = {
  gist: string;
  what?: string | null;
  checks: { status?: string | null; text: string }[];
  left?: string | null;
  since?: SinceReturn | null;
};

function HandInBlocks({ gist, what, checks, left, since }: Parts) {
  return (
    <div className="report hand-in" data-testid="agent-report">
      <div data-testid="handin-changed">
        <h5>What changed</h5>
        <p className="report-gist">{gist}</p>
        {what && <p>{what}</p>}
      </div>
      <div data-testid="handin-checks">
        <h5>Checks</h5>
        <ul className="checks">
          {checks.map((c, i) => (
            <li key={i} data-status={c.status ?? undefined}>
              {c.status ? <><span className="status">{c.status}</span> {c.text}</> : c.text}
            </li>
          ))}
        </ul>
      </div>
      <div data-testid="handin-left">
        <h5>What is left</h5>
        <p>{left ?? "Nothing was named."}</p>
      </div>
      {since && <SinceReturnBlock since={since} />}
    </div>
  );
}
