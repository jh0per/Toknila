// Text for people: status reasons and usage figures.

import type { OutOfOrder, StatusReason, UsageFigures } from "./api/types";

/// Said wherever a cost is shown: the numbers are prices times tokens, not a bill.
export const ESTIMATE_NOTE = "Estimate at API list prices; on a subscription the real cost differs.";

/// A token count in few characters: 842, 12.3k, 1.2M, 3.4B.
export function tokensText(n: number): string {
  const scaled = (div: number, unit: string) => `${(n / div).toFixed(1).replace(/\.0$/, "")}${unit}`;
  if (n >= 1e9) return scaled(1e9, "B");
  if (n >= 1e6) return scaled(1e6, "M");
  if (n >= 1e4) return scaled(1e3, "k");
  return String(Math.round(n));
}

/// Dollars with cents; amounts under a cent are "<$0.01".
export function costText(usd: number): string {
  if (usd > 0 && usd < 0.01) return "<$0.01";
  return `$${usd.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

/// A share from 0 to 1 as a whole percentage.
export const shareText = (share: number): string => `${Math.round(share * 100)}%`;

/// The small chip on a card: tokens and estimated cost; empty when the card used nothing.
export function chipText(f: Pick<UsageFigures, "processed" | "cost_usd" | "messages"> | undefined): string {
  if (!f || f.messages === 0) return "";
  return `${tokensText(f.processed)} tok · ~${costText(f.cost_usd)}`;
}

export function reasonText(r: StatusReason | null | undefined): string {
  if (!r) return "";
  switch (r.status) {
    case "waiting":
      if (r.kind === "screen") return "Claude Code asks on its screen";
      return r.kind === "silent" ? "agent went silent" : `${r.kind}: ${r.text}`;
    case "cancelled":
      return `cancelled: ${r.text}`;
    case "proposed":
      return "awaiting approval";
    case "blocked": {
      const b = r.reason;
      switch (b.kind) {
        case "manual": return `blocked: ${b.text}`;
        case "crashed": return `crashed: ${b.detail}`;
        case "terminated": return `the agent was terminated from outside (${b.detail}); unblock starts it again`;
        case "dependency_reopened": return `dependency reopened: ${b.dependency}`;
        case "merge_conflict": return `merge conflict: ${b.detail}`;
        case "base_checkout_dirty": return `${b.path} has local changes; commit or stash them, then unblock`;
        case "ship_uncommitted": return `the workspace has changes that are not committed (${b.files.slice(0, 8).join(", ")}${b.files.length > 8 ? ` and ${b.files.length - 8} more` : ""}); commit them in the workspace, then unblock`;
        case "ship_empty": return "the workspace has no commit of its own, so there is nothing to merge; commit the work in the workspace, then unblock";
        case "merge_failed": return `merge failed: ${b.detail}`;
        case "ci_failed": return `checks failed: ${b.checks.join(", ")}`;
        case "pr_closed": return "the pull request was closed without merging";
        case "runner_lost": return "runner lost";
        case "ship_failed": return `workspace failed: ${b.detail}`;
        case "setup_failed": return `setup failed: ${b.command}${b.output ? `\n${b.output}` : ""}`;
        case "base_behind": return `main branch ${b.base} is ${b.behind} ${b.behind === 1 ? "commit" : "commits"} behind ${b.upstream}${b.can_fast_forward ? "; fast-forward it and start, " : "; "}start anyway, or unblock to look again`;
        case "no_working_dir": return "no working directory: the project has no repository; give it a folder or use the simulated agent";
      }
    }
  }
  return "";
}

/// The mark of a card done out of order, naming the earlier cards that were still open.
export function outOfOrderText(mark: OutOfOrder | null | undefined): string {
  if (!mark) return "";
  return `done out of order: ${mark.behind.join(", ")} still open`;
}
