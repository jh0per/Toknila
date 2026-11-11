import type { Stray } from "../api/types";

/// One leftover as a line of the list: a workspace by its path, a branch by its name and the
/// work that deleting it would lose.
export function strayText(s: Stray): string {
  if (s.kind === "worktree") return s.branch ? `${s.name} (branch ${s.branch})` : s.name;
  const owner = [s.card ? `card ${s.card}` : "", s.merge_commit ? `merged in ${s.merge_commit}` : ""]
    .filter(Boolean)
    .join(", ");
  const tail = owner ? ` [${owner}]` : "";
  if (s.ahead === undefined || s.ahead === null) return `${s.name}${tail}`;
  if (s.ahead === 0) return `${s.name} (no commits of its own)${tail}`;
  return `${s.name} (${s.ahead} commit${s.ahead === 1 ? "" : "s"} not in the base)${tail}`;
}

/// What Clean up reports when it is done.
export function cleanedText(removed: number, warnings: number): string {
  const what = `${removed} leftover${removed === 1 ? "" : "s"} removed`;
  return warnings > 0 ? `${what}; ${warnings} could not be removed` : what;
}
