import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { InboxItem } from "../api/types";
import { initialState } from "../store/state";
import { InboxRow } from "./InboxRow";

const state = initialState({ name: "inbox" } as never);

const item = (extra: Partial<InboxItem>) =>
  ({
    kind: "review", ticket: "DEMO-1", chain: "c1", title: "Alpha", since: 1, text: "", thread: null,
    subtasks: [], done_cards: [], allowed_actions: ["accept", "return"], ...extra,
  }) as InboxItem;

const render = (i: InboxItem) => renderToStaticMarkup(<InboxRow state={state} item={i} dispatch={() => {}} />);

describe("InboxRow", () => {
  it("shows the hand-in of a review from the item alone, with no card details loaded", () => {
    expect(state.details).toEqual({});
    const html = render(
      item({
        hand_in: {
          gist: "Alpha is built",
          what: "InboxItem got a hand-in",
          checks: [{ status: "PASS", text: "verify: green" }, { status: "FAIL", text: "e2e: red" }, { status: "NOT_RUN", text: "web: no browser" }],
          left: "docs",
          since_return: { round: 1, from: "aaa1111bbb", to: "ccc2222ddd", commits: ["fix the empty list"], files: ["a.rs"] },
        },
      }),
    );
    expect(html).toMatch(/data-testid="handin-changed"[^>]*>.*Alpha is built.*InboxItem got a hand-in/);
    expect(html).toMatch(/data-testid="handin-checks".*data-status="PASS".*PASS<\/span> verify: green.*data-status="FAIL".*FAIL<\/span> e2e: red.*data-status="NOT_RUN"/);
    expect(html).toMatch(/data-testid="handin-left"[^>]*>.*docs/);
    expect(html).toMatch(/data-testid="handin-since".*Since the Return.*fix the empty list/);
    expect(html).toContain('data-testid="review-accept"');
  });

  it("shows a question's text and a pin's design name from the item", () => {
    expect(render(item({ kind: "question", text: "Which flag?", answers: [], allowed_actions: [] }))).toContain("Which flag?");
    const pin = render(item({ kind: "design_question", text: "home.png: Is the logo fixed?", design: "home.png", pin: "p1", allowed_actions: [] }));
    expect(pin).toContain("home.png: Is the logo fixed?");
  });

  it("shows a pin no card owns by its design, answered in place, with no card to open", () => {
    const html = render(
      item({
        kind: "pin", ticket: undefined, chain: undefined, project: "p1", title: "home.png", text: "Which logo?",
        design: "home.png", pin: "pin1", allowed_actions: [],
      }),
    );
    expect(html).toContain('data-kind="pin"');
    expect(html).toContain("The assistant asks about home.png");
    expect(html).toContain("Which logo?");
    expect(html).toContain('data-testid="inbox-pin-input"');
    expect(html).toContain('data-testid="inbox-pin-send"');
    expect(html).not.toContain("inbox-open-card");
    expect(html).not.toContain("DEMO-1");
  });
});
