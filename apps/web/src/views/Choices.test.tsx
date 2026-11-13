import { isValidElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { InboxItem } from "../api/types";
import { Answer } from "./Answer";
import { Choices, choiceIntent } from "./Choices";

const CHOICES = ["Hand in now; you check on the phone at review", "Wait; I check on the phone first and tell you"];

/// Every element of a rendered tree whose type is the tag `tag`.
function find(node: ReactNode, tag: string, out: ReactElement<{ onClick?: () => void; children?: ReactNode }>[] = []) {
  if (Array.isArray(node)) node.forEach((n) => find(n, tag, out));
  else if (isValidElement<{ children?: ReactNode }>(node)) {
    if (node.type === tag) out.push(node as never);
    find(node.props.children, tag, out);
  }
  return out;
}

describe("Choices", () => {
  it("shows one button per choice, the first marked recommended, in the order given", () => {
    const html = renderToStaticMarkup(<Choices choices={CHOICES} onPick={() => {}} />);
    const buttons = [...html.matchAll(/<button[^>]*data-testid="choice-(\d)"[^>]*>(.*?)<\/button>/g)];
    expect(buttons.map((m) => m[1])).toEqual(["0", "1"]);
    expect(buttons[0][2]).toContain(CHOICES[0]);
    expect(buttons[0][2]).toContain("recommended");
    expect(buttons[1][2]).toContain(CHOICES[1]);
    expect(buttons[1][2]).not.toContain("recommended");
  });

  it("answers at once with the index of the pressed button", () => {
    const onPick = vi.fn();
    const buttons = find(Choices({ choices: CHOICES, onPick }), "button");
    expect(buttons).toHaveLength(2);
    buttons[1].props.onClick?.();
    expect(onPick.mock.calls).toEqual([[1]]);
  });

  it("turns a pick into the answer intent the store sends: text with the choice index", () => {
    expect(choiceIntent("DEMO-1", "worker", 1)).toEqual({ type: "answer", key: "DEMO-1", role: "worker", answer: "text", choice: 1 });
  });

  it("is part of a question's panel, above the field for another answer, and absent without choices", () => {
    const item = (choices?: string[]) =>
      ({ kind: "question", ticket: "DEMO-1", title: "t", text: "Check the phone?", role: "worker", answers: [], choices }) as unknown as InboxItem;
    const html = renderToStaticMarkup(<Answer item={item(CHOICES)} dispatch={() => {}} />);
    expect(html.indexOf('data-testid="choice-0"')).toBeGreaterThan(-1);
    expect(html.indexOf('data-testid="choice-0"')).toBeLessThan(html.indexOf('aria-label="Reply to DEMO-1"'));
    expect(html).toContain("Or write another answer");
    expect(renderToStaticMarkup(<Answer item={item()} dispatch={() => {}} />)).not.toContain("choice-0");
  });
});
