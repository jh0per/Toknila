import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import type { ConversationMessage } from "../api/types";
import { readFileSync } from "node:fs";
import { DEFAULT_WIDTH, MIN_WIDTH } from "../store/assistant";
import { initialState } from "../store/state";
import { ChatEntry } from "./ChatMessage";

beforeAll(() => { vi.stubGlobal("window", { location: { search: "" } }); });

const assistantCss = readFileSync(new URL("../assistant.css", import.meta.url), "utf8");
const tokensCss = readFileSync(new URL("../tokens.css", import.meta.url), "utf8");
/// The declarations of the rule whose selector is exactly `selector`.
const rule = (sheet: string, selector: string): string => {
  const at = sheet.split("\n").find((l) => l.startsWith(`${selector} {`));
  if (!at) throw new Error(`no rule for ${selector}`);
  return at.slice(at.indexOf("{") + 1, at.lastIndexOf("}"));
};

const state = initialState({ name: "project", project: "DEMO" } as never);
const say = (author: "person" | "assistant" | "toknila", text: string): ConversationMessage =>
  ({ id: `m-${author}`, author, text, at: 1 }) as ConversationMessage;
const html = (m: ConversationMessage) => renderToStaticMarkup(<ul><ChatEntry m={m} project="DEMO" state={state} dispatch={() => {}} /></ul>);

describe("the chat's rows", () => {
  const sheet = assistantCss;

  it("gives the person's and the assistant's bubbles one padding class, and that class one padding and radius", () => {
    for (const a of ["person", "assistant"] as const) expect(html(say(a, "hello"))).toMatch(/class="bubble msg chat-bubble (you|agent)"/);
    const shared = rule(sheet, ".bubble.chat-bubble");
    expect(shared).toContain("padding: var(--sp3) var(--sp4)");
    expect(shared).toContain("border-radius: var(--shape-m)");
    expect(shared).toContain("max-width: 85%");
    // Neither voice's rule sets a padding or radius of its own.
    for (const voice of [".bubble.chat-bubble.you", ".bubble.chat-bubble.agent"]) expect(rule(sheet, voice)).not.toMatch(/padding|radius/);
    expect(rule(sheet, ".bubble.chat-bubble.you")).toContain("var(--primary-container)");
    expect(rule(sheet, ".bubble.chat-bubble.agent")).toContain("var(--surface-container-high)");
    expect(rule(sheet, ".bubble.chat-bubble .gist")).toMatch(/white-space: pre-wrap/);
  });

  it("never wraps the time of a bubble head or of a did row", () => {
    expect(html(say("person", "hello"))).toMatch(/<div class="bubble-head"><b>You<\/b>.*<time /);
    expect(html(say("toknila", "Started DEMO-1"))).toMatch(/<li class="did"[^>]*><span class="did-text">Started DEMO-1<\/span><time /);
    for (const selector of [".bubble.chat-bubble .bubble-head time", ".did time"]) {
      const r = rule(sheet, selector);
      expect(r).toContain("flex: none");
      expect(r).toContain("white-space: nowrap");
    }
    expect(rule(sheet, ".did .did-text")).toContain("min-width: 0");
  });

  it("holds the panel widths of the tokens", () => {
    const tokens = tokensCss;
    expect(tokens).toContain(`--assistant-w-default: ${DEFAULT_WIDTH}px`);
    expect(tokens).toContain(`--assistant-w-min: ${MIN_WIDTH}px`);
    expect([DEFAULT_WIDTH, MIN_WIDTH]).toEqual([440, 320]);
  });
});
