import type { Ship, Ticket } from "../api/types";
import { outOfOrderText } from "../format";
import { PullRequestInfo } from "./PrChip";

/// The out-of-order mark and the pull request of a card, when it has them.
export function DetailsMeta({ card }: { card: Ticket }) {
  if (!card.done_out_of_order && !card.pr) return null;
  return (
    <div className="details-meta">
      {card.done_out_of_order && (
        <p className="tag-note" data-testid="out-of-order">{outOfOrderText(card.done_out_of_order)}</p>
      )}
      {card.pr && <PullRequestInfo pr={card.pr} />}
    </div>
  );
}

/// What only a programmer needs, folded away: the card's workspace (its folder and branch) and the
/// agent it runs with. The agent's name is the card's own, else the project's default.
export function TechnicalDetails({ card, ship, projectDefault }: {
  card: Ticket;
  ship: Ship | null | undefined;
  projectDefault: string | undefined;
}) {
  const agent = card.profile ?? projectDefault;
  if (!ship && !agent) return null;
  return (
    <details className="fold technical" data-testid="technical-details">
      <summary>Technical details</summary>
      <dl className="facts" data-testid="ship">
        {ship && (
          <>
            <dt>Workspace</dt><dd data-testid="ship-path"><code>{ship.path}</code></dd>
            <dt>Branch</dt><dd data-testid="ship-branch"><code>{ship.branch}</code></dd>
          </>
        )}
        {agent && (
          <>
            <dt>Agent</dt>
            <dd data-testid="card-agent"><code>{agent}</code>{card.profile ? "" : " (the project's default)"}</dd>
          </>
        )}
      </dl>
    </details>
  );
}
