// Older cards carry a report comment for every permission request an agent made ("Claude Code
// asks for permission: Bash: ..."). Permission requests now live in the journal and the Inbox
// only; those old comments are left out of the list (nothing is migrated).

import type { Comment } from "../api/types";

const PERMISSION = /^Claude (Code )?(asks for|needs your) permission/i;

/// Whether the comment is an agent's report of a permission request.
export function isPermissionReport(c: Comment): boolean {
  return c.kind === "report" && c.author === "agent" && PERMISSION.test(c.body.gist);
}

/// The agent's last written report that is not a permission request: what it says it did when it
/// handed in (or why a reviewer could not decide).
export function finishReport(comments: Comment[]): Comment | undefined {
  return [...comments].reverse().find((c) => c.kind === "report" && c.author === "agent" && !isPermissionReport(c));
}
