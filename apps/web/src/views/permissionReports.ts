// Older cards carry a report comment for every permission request an agent made ("Claude Code
// asks for permission: Bash: ..."). Permission requests now live in the journal and the Inbox
// only; those old comments are left out of the list (nothing is migrated).

import type { Comment } from "../api/types";

const PERMISSION = /^Claude (Code )?(asks for|needs your) permission/i;

/// Whether the comment is an agent's report of a permission request.
export function isPermissionReport(c: Comment): boolean {
  return c.kind === "report" && c.author === "agent" && PERMISSION.test(c.body.gist);
}
