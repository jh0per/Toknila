import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ThreadDetail } from "../api/types";
import { askLine, awaitingAnswer, diffLines, diffText, entries } from "../store/discussion";
import { DiffView } from "./DiffView";

describe("the diff of a proposed edit", () => {
  it("marks the removed line, the added lines and the kept ones in reading order", () => {
    expect(diffLines(["one", "two"], ["one", "two!", "three"])).toEqual([
      { op: "same", text: "one" },
      { op: "del", text: "two" },
      { op: "add", text: "two!" },
      { op: "add", text: "three" },
    ]);
    expect(diffText("a\nb", "a\nb")).toEqual([{ op: "same", text: "a" }, { op: "same", text: "b" }]);
  });

  it("shows each changed field with minus and plus lines, and says so for a field it leaves alone", () => {
    const html = renderToStaticMarkup(
      <DiffView proposal={{ acceptance: { from: ["Errors are shown"], to: ["Errors are shown with the server message", "A 4xx keeps the form"] } }} />,
    );
    expect(html).toContain('data-testid="edit-diff"');
    expect(html).toContain('<span class="diff-line del" data-op="del">- Errors are shown\n</span>');
    expect(html).toContain('<span class="diff-line add" data-op="add">+ A 4xx keeps the form\n</span>');
    expect(html).toMatch(/data-field="prompt"><h5>Prompt<\/h5><p class="muted">No change<\/p>/);
  });
});

const msg = (id: string, kind: string, author: string, text: string) => ({ id, kind, author, text, thread_id: "T", created_at: 0 });
const thread = (messages: ReturnType<typeof msg>[]) => ({ thread: { kind: "discussion", number: 1 }, messages, versions: [] }) as unknown as ThreadDetail;
const proposal = JSON.stringify({ prompt: { from: "a", to: "b" } });

describe("the discussion read from its thread", () => {
  it("turns messages and proposals into entries, a proposal pending until a later message names it", () => {
    const pending = thread([msg("m1", "message", "human", "stricter"), msg("m2", "edit_proposal", "agent", proposal)]);
    expect(entries(pending).map((e) => (e.type === "edit" ? `edit:${e.state}` : `say:${e.side}`))).toEqual(["say:you", "edit:pending"]);
    const accepted = thread([...pending.messages, msg("m3", "edit_accepted", "human", "m2")]);
    expect(entries(accepted)[1]).toMatchObject({ type: "edit", state: "accepted" });
    const rejected = thread([...pending.messages, msg("m3", "edit_rejected", "human", "m2")]);
    expect(entries(rejected)[1]).toMatchObject({ type: "edit", state: "rejected" });
  });

  it("waits for an answer while the person spoke last, and not once the assistant said something", () => {
    expect(awaitingAnswer(undefined)).toBe(false);
    expect(awaitingAnswer(thread([msg("m1", "message", "human", "hi")]))).toBe(true);
    expect(awaitingAnswer(thread([msg("m1", "message", "human", "hi"), msg("m2", "message", "agent", "hello")]))).toBe(false);
    expect(awaitingAnswer(thread([msg("m1", "message", "human", "hi"), msg("m2", "edit_proposal", "agent", proposal)]))).toBe(false);
  });

  it("types the planner one line that names the card, the thread and the words", () => {
    expect(askLine("DEMO-1", 2, "  make it\nstricter ")).toBe("Discussion on card DEMO-1 (thread 2): make it stricter");
  });
});
