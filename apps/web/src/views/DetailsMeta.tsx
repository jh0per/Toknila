import type { Ship, Ticket } from "../api/types";
import { outOfOrderText } from "../format";
import { PullRequestInfo } from "./PrChip";

/// The out-of-order mark and the ship (worktree and branch) of a card, when it has them.
export function DetailsMeta({ card, ship }: { card: Ticket; ship: Ship | null | undefined }) {
  if (!card.done_out_of_order && !ship && !card.pr) return null;
  return (
    <div className="details-meta">
      {card.done_out_of_order && (
        <p className="tag-note" data-testid="out-of-order">{outOfOrderText(card.done_out_of_order)}</p>
      )}
      {card.pr && <PullRequestInfo pr={card.pr} />}
      {ship && (
        <dl className="ship" data-testid="ship">
          <dt>Worktree</dt><dd data-testid="ship-path">{ship.path}</dd>
          <dt>Branch</dt><dd data-testid="ship-branch">{ship.branch}</dd>
        </dl>
      )}
    </div>
  );
}
