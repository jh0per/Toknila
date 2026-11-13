import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Comment, ReviewRound, ReviewView } from "../api/types";
import { NeedsYou, ReviewHint, ReviewLines } from "./NeedsYou";

const report = (extra: object): Comment =>
  ({ id: "c", ticket_id: "t", kind: "report", author: "agent", body: { gist: "Built it", ...extra }, created_at: 1, delivered_at: null }) as unknown as Comment;
const view = (comment: string) =>
  ({ rounds: [{ number: 1, reviewer: "agent", verdict: "request_changes", comment, decided_at: 0 } as ReviewRound] }) as unknown as ReviewView;

describe("Needs you", () => {
  it("lists the PERSON items of the hand-in and of the verdict above the chat", () => {
    const html = renderToStaticMarkup(
      <NeedsYou report={report({ how_to_verify: "- PERSON look at the picture" })} review={view("PERSON 2. Looks right on a phone [read]")} />,
    );
    expect(html).toContain('data-testid="needs-you"');
    expect(html).toContain("Needs you");
    expect(html).toContain("<li>look at the picture</li>");
    expect(html).toContain("<li>Looks right on a phone</li>");
    expect(html).not.toContain("Nothing to do by hand");
  });

  it("says nothing is to be done by hand when no item is for a person", () => {
    const html = renderToStaticMarkup(<NeedsYou report={report({ how_to_verify: "- PASS a: ok" })} review={view("PASS 1. fine [ran]")} />);
    expect(html).toContain("Nothing to do by hand; Accept when the hand-in convinces you");
    expect(html).not.toContain("<li>");
  });
});

describe("the first-time hint", () => {
  it("is two sentences above the buttons, with a way to dismiss it", () => {
    const html = renderToStaticMarkup(<ReviewHint project="DEMO" />);
    expect(html).toContain("Accept merges the story. Return sends your words to the agent, which fixes and hands in again.");
    expect(html).toContain('data-testid="review-hint-dismiss"');
  });
});

describe("the line under each button", () => {
  it("says what Accept and Return do", () => {
    const html = renderToStaticMarkup(<ReviewLines />);
    expect(html).toContain("Accept: the story merges");
    expect(html).toContain("Return: the agent fixes what you found");
  });
});
