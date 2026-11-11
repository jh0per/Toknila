import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { InboxItem } from "../api/types";
import { Answer } from "./Answer";

const item = (extra: Partial<InboxItem>) =>
  ({
    kind: "question", ticket: "DEMO-1", title: "t", text: "Claude Code asks for permission: Bash: python3 - EOF print(1) EOF",
    answers: ["allow", "deny"], role: "worker", ...extra,
  }) as InboxItem;

describe("Answer", () => {
  it("offers the whole command behind a closed disclosure when the list text was cut", () => {
    const html = renderToStaticMarkup(<Answer item={item({ detail: "Bash: python3 - <<EOF\nprint(1)\nEOF" })} dispatch={() => {}} />);
    expect(html).toMatch(/<details[^>]*data-testid="question-detail"[^>]*>\s*<summary>Show the full command<\/summary>/);
    expect(html).not.toContain("<details open");
    expect(html).toContain("print(1)\nEOF</pre>");
  });
  it("offers Allow always next to Allow for a worker's command, and for nothing else", () => {
    const html = (extra: Partial<InboxItem>) => renderToStaticMarkup(<Answer item={item(extra)} dispatch={() => {}} />);
    const bash = html({ allow_always: "Bash(cargo test *)" });
    expect(bash).toMatch(/data-testid="answer-allow"[^>]*>Allow<\/button><button[^>]*data-testid="answer-allow-always"[^>]*>Allow always</);
    expect(bash).toContain("Allow always: Bash(cargo test *)");
    expect(html({ text: "Claude Code asks for permission: Write: src/lib.rs" })).not.toContain("answer-allow-always");
    expect(html({ role: "reviewer", allow_always: "Bash(ls *)" })).not.toContain("answer-allow-always");
    expect(html({ answers: ["enter"], allow_always: "Bash(ls *)" })).not.toContain("answer-allow-always");
  });
  it("hides Allow always and says why when the command cannot become a rule", () => {
    const out = renderToStaticMarkup(
      <Answer item={item({ allow_always_none: "a command built with $ or backticks cannot become a rule" })} dispatch={() => {}} />,
    );
    expect(out).not.toContain('data-testid="answer-allow-always"');
    expect(out).toContain("Allow always is not offered: a command built with $ or backticks cannot become a rule");
    expect(out).toContain('data-testid="answer-allow"');
  });
  it("has no disclosure when the text already says everything", () => {
    const html = renderToStaticMarkup(<Answer item={item({})} dispatch={() => {}} />);
    expect(html).not.toContain("question-detail");
    expect(html).not.toContain("Show the full command");
  });
});
