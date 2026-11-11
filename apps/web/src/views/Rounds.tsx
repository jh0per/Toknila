import type { TicketDetail } from "../api/types";
import { awaitingText, basisText, itemRows, loopText, roundText, whenText } from "../store/review";

/// The acceptance items of a verdict, each with whether the reviewer ran what shows it or only read.
function RoundItems({ comment }: { comment: string }) {
  const rows = itemRows(comment);
  if (rows.length === 0) return null;
  return (
    <ul className="round-items" data-testid="round-items">
      {rows.map((row) => (
        <li key={row.item} data-testid="round-item" data-outcome={row.outcome} data-basis={row.basis}>
          <span className="status">{row.outcome}</span> {row.item}. {row.text}{" "}
          <span className="basis" data-testid="item-basis">{basisText(row.basis)}</span>
        </li>
      ))}
    </ul>
  );
}

/// The card's review: whose verdict it waits for, the rounds of the loop used, and each round so
/// far (who, verdict, when), as the daemon holds them.
export function Rounds({ detail }: { detail?: TicketDetail }) {
  const review = detail?.review;
  if (!review || (review.rounds.length === 0 && !review.awaiting)) return null;
  const waiting = awaitingText(review);
  return (
    <section className="rounds" data-testid="rounds">
      <h4>Review</h4>
      {waiting && <p className="muted" data-testid="awaiting">{waiting}</p>}
      <p className="muted" data-testid="loop-rounds">{loopText(review)}</p>
      <ul>
        {review.rounds.map((r) => (
          <li key={r.number} data-testid="round">
            {roundText(r)} <time className="muted" dateTime={new Date(r.decided_at).toISOString()}>{whenText(r.decided_at)}</time>
            <RoundItems comment={r.comment} />
          </li>
        ))}
      </ul>
    </section>
  );
}
