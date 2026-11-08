// The new-card form: the person says what the agent should do, the title follows from it until
// they write one themselves, and the acceptance items are typed one per line.

const MAX_TITLE = 70;

/// A title from the first line of the task: Markdown marks (heading hashes, bullets, numbers)
/// dropped, spaces collapsed, cut at a word boundary with an ellipsis when long. Empty for an
/// empty task.
export function titleFromTask(task: string): string {
  const first = task.split("\n").map((l) => l.trim()).find((l) => l !== "") ?? "";
  const plain = first.replace(/^(#{1,6}\s+|[-*+]\s+|\d+[.)]\s+)/, "").replace(/\s+/g, " ").trim();
  if (plain.length <= MAX_TITLE) return plain;
  const cut = plain.slice(0, MAX_TITLE);
  const at = cut.lastIndexOf(" ");
  return `${(at > MAX_TITLE / 2 ? cut.slice(0, at) : cut).replace(/[\s,;:.-]+$/, "")}…`;
}

/// The acceptance items of a textarea: one per non-empty line, a leading bullet dropped.
export function acceptanceFrom(text: string): string[] {
  return text
    .split("\n")
    .map((l) => l.trim().replace(/^[-*+]\s+/, "").trim())
    .filter((l) => l !== "");
}

/// The title to send: the one the person wrote, else (nothing written, or cleared) the one
/// derived from the task.
export function cardTitle(typed: string | undefined, task: string): string {
  return (typed ?? "").trim() || titleFromTask(task);
}
