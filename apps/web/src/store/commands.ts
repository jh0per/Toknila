// Words for the commands setting and the rules "Allow always" remembers. The rule itself is made
// by the daemon (core derives it from the command); the window only shows it and offers it.

import type { CommandPolicy, InboxItem } from "../api/types";

/// The three choices of "When the agent wants to run a command", one plain line each.
export const COMMAND_CHOICES: { value: CommandPolicy; label: string; hint: string }[] = [
  { value: "auto", label: "Let Claude decide", hint: "Claude Code decides and asks you only about risky actions." },
  { value: "ask", label: "Ask me each time", hint: "You answer a question for every command that is not already allowed." },
  { value: "trust", label: "Never ask", hint: "The agent runs any command. Use it only for a project you trust." },
];

/// The command inside a remembered rule: `Bash(cargo test *)` is `cargo test *`.
export function ruleWords(rule: string): string {
  const m = /^Bash\((.*)\)$/s.exec(rule);
  return m ? m[1] : rule;
}

/// Whether "Allow always" is offered for a question: a worker's permission prompt about a command
/// (the text names it as `Bash: <command>`).
export function canAllowAlways(item: Pick<InboxItem, "role" | "answers" | "text" | "detail">): boolean {
  if ((item.role ?? "worker") !== "worker" || !(item.answers ?? []).includes("allow")) return false;
  return /(^|[\s:])Bash: \S/.test(item.detail ?? "") || /(^|[\s:])Bash: \S/.test(item.text);
}
