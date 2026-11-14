import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { WaitingLine } from "./WaitingLine";

beforeAll(() => { vi.stubGlobal("window", { location: { search: "" } }); });

const MIN = 60_000;
const since = 1_000_000;
const html = (minutes: number, progress?: string) =>
  renderToStaticMarkup(
    <WaitingLine
      project="DEMO" since={since} now={since + minutes * MIN + 5_000} dispatch={() => {}}
      progress={progress === undefined ? undefined : { id: "p1", author: "progress", text: progress, at: since + 1 }}
    />,
  );
const text = (markup: string) => markup.replace(/<[^>]+>/g, "");

describe("the waiting line", () => {
  it("says the assistant is answering, without a count at the start", () => {
    expect(text(html(0))).toBe("The assistant is answering…");
  });

  it("counts the minutes while it answers", () => {
    expect(text(html(2))).toBe("The assistant is answering… 2 min");
    expect(text(html(4))).toBe("The assistant is answering… 4 min");
  });

  it("after five minutes says it is still working and points to the assistant's terminal", () => {
    const out = html(5);
    expect(text(out)).toBe("The assistant is still working (5 min); its terminal is under Assistant → Terminal");
    expect(out).toMatch(/<a [^>]*data-testid="chat-terminal-link"[^>]*href="\/projects\/DEMO\/planner"/);
    expect(text(html(12))).toContain("still working (12 min)");
  });

  it("shows the assistant's last progress line under the count, at every stage", () => {
    for (const [minutes, line] of [[0, "answering…"], [2, "answering… 2 min"], [5, "still working (5 min)"]] as const) {
      const out = html(minutes, "Reading the daemon's code, a few minutes");
      expect(out).toMatch(/data-testid="chat-progress"[^>]*>Reading the daemon&#x27;s code, a few minutes</);
      expect(text(out)).toContain(line);
      expect(out.indexOf("chat-progress")).toBeGreaterThan(out.indexOf("chat-waiting-text"));
    }
  });

  it("shows no progress line when the assistant has said none", () => {
    for (const minutes of [0, 2, 5]) expect(html(minutes)).not.toContain("chat-progress");
  });
});
