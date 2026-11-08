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
  it("has no disclosure when the text already says everything", () => {
    const html = renderToStaticMarkup(<Answer item={item({})} dispatch={() => {}} />);
    expect(html).not.toContain("question-detail");
    expect(html).not.toContain("Show the full command");
  });
});
