import type { PullRequest } from "../api/types";

/// How the checks of a pull request add up, for the chip: failed beats pending beats passed.
export function checksSummary(pr: PullRequest): "failed" | "pending" | "passed" {
  if (pr.checks.some((c) => c.state === "failed")) return "failed";
  if (pr.checks.length === 0 || pr.checks.some((c) => c.state === "pending")) return "pending";
  return "passed";
}

/// The pull request of a card on the canvas: a chip that links to it. The card is itself a link,
/// so the chip opens the page from its own click and never navigates the card.
export function PrChip({ pr }: { pr: PullRequest }) {
  const open = () => window.open(pr.url, "_blank", "noopener,noreferrer");
  return (
    <span
      className={`mark pr ${checksSummary(pr)}`}
      role="link"
      tabIndex={0}
      data-testid="pr-chip"
      data-href={pr.url}
      title={`${pr.url}: checks ${checksSummary(pr)}`}
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        open();
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          e.stopPropagation();
          open();
        }
      }}
    >
      PR #{pr.number}
    </span>
  );
}

/// The pull request in the card's details: the link and each check with its state.
export function PullRequestInfo({ pr }: { pr: PullRequest }) {
  return (
    <div className="pr" data-testid="pr">
      <a href={pr.url} target="_blank" rel="noreferrer" data-testid="pr-url">Pull request #{pr.number}</a>
      <ul className="checks" data-testid="pr-checks">
        {pr.checks.map((c) => (
          <li key={c.name} data-state={c.state}>{c.name}: {c.state}</li>
        ))}
        {pr.checks.length === 0 && <li className="muted">no checks reported yet</li>}
      </ul>
    </div>
  );
}
