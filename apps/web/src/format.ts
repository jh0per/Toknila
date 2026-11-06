// Text for people: status reasons and durations.

import type { OutOfOrder, StatusReason } from "./api/types";

export function reasonText(r: StatusReason | null | undefined): string {
  if (!r) return "";
  switch (r.status) {
    case "waiting":
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
        case "dependency_reopened": return `dependency reopened: ${b.dependency}`;
        case "merge_conflict": return `merge conflict: ${b.detail}`;
        case "base_checkout_dirty": return `${b.path} has local changes; commit or stash them, then unblock`;
        case "merge_failed": return `merge failed: ${b.detail}`;
        case "ci_failed": return `checks failed: ${b.checks.join(", ")}`;
        case "pr_closed": return "the pull request was closed without merging";
        case "runner_lost": return "runner lost";
        case "ship_failed": return `ship failed: ${b.detail}`;
        case "no_working_dir": return "no working directory: the project has no repository; give it one or use the sim profile";
      }
    }
  }
  return "";
}

export function duration(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 48) return `${h}h ${m % 60}m`;
  return `${Math.floor(h / 24)}d`;
}

/// The mark of a card done out of order, naming the earlier cards that were still open.
export function outOfOrderText(mark: OutOfOrder | null | undefined): string {
  if (!mark) return "";
  return `done out of order: ${mark.behind.join(", ")} still open`;
}
