// Words for the commands setting and the rules "Allow always" remembers. The rule itself is made
// by the daemon (core derives it from the command); the window only shows it and offers it.

import type { CommandPolicy, InboxItem } from "../api/types";

/// The three choices of "When the agent wants to run a command", one plain line each.
export const COMMAND_CHOICES: { value: CommandPolicy; label: string; hint: string }[] = [
  { value: "auto", label: "Let Claude decide", hint: "Claude Code decides and asks you only about risky actions." },
  { value: "ask", label: "Ask me each time", hint: "You answer a question for every command that is not already allowed." },
  { value: "trust", label: "Never ask", hint: "The agent runs any command and uses any tool, including tools from other servers, without asking. Only what Toknila always refuses stays refused: starting other agents, scheduling work outside the card, leaving the workspace and reading your private folders. Use it only for a project you trust." },
];

/// The command inside a remembered rule: `Bash(cargo test *)` is `cargo test *`.
export function ruleWords(rule: string): string {
  const m = /^Bash\((.*)\)$/s.exec(rule);
  return m ? m[1] : rule;
}

/// The rule "Allow always" would add for a question, which the daemon made (core derives it from
/// the command); `undefined` when the button must not be offered: no rule can be made, the
/// question is not about a command, it is a reviewer's, or Allow is not an answer to it.
export function allowAlwaysRule(item: Pick<InboxItem, "role" | "answers" | "allow_always">): string | undefined {
  if ((item.role ?? "worker") !== "worker" || !(item.answers ?? []).includes("allow")) return undefined;
  return item.allow_always || undefined;
}

/// The line under the button: the rule it adds, or why there is none.
export function allowAlwaysLine(item: Pick<InboxItem, "role" | "answers" | "allow_always" | "allow_always_none">): string | undefined {
  const rule = allowAlwaysRule(item);
  if (rule) return `Allow always: ${rule}`;
  if ((item.role ?? "worker") !== "worker" || !(item.answers ?? []).includes("allow")) return undefined;
  return item.allow_always_none ? `Allow always is not offered: ${item.allow_always_none}` : undefined;
}
