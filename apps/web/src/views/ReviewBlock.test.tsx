import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { TicketDetail } from "../api/types";
import { ReviewBlock } from "./ReviewBlock";
import { Rounds } from "./Rounds";

const detail = (withdrawn: number | null, by: string) =>
  ({
    comments: [],
    ticket: { review: withdrawn === null ? {} : { accept_withdrawn: withdrawn } },
    review: {
      mode: "both",
      reviewer: "x",
      awaiting: { by },
      max_rounds: 3,
      used_rounds: 1,
      rounds: [{ number: 2, reviewer: "agent", verdict: "request_changes", comment: "red", decided_at: 0 }],
    },
  }) as unknown as TicketDetail;

describe("an Accept voided by the reviewer", () => {
  it("tells the person at their turn to look again", () => {
    const html = renderToStaticMarkup(<ReviewBlock detail={detail(2, "person")} project="DEMO">{null}</ReviewBlock>);
    expect(html).toContain("Your Accept was before the reviewer asked for changes; look again");
  });

  it("says nothing for a card whose Accept stands", () => {
    const html = renderToStaticMarkup(<ReviewBlock detail={detail(null, "person")} project="DEMO">{null}</ReviewBlock>);
    expect(html).not.toContain("accept-before-changes");
  });

  it("words the withdrawal under the round that caused it", () => {
    const html = renderToStaticMarkup(<Rounds detail={detail(2, "person")} />);
    expect(html).toContain("Accept withdrawn: the reviewer asked for changes after it");
  });
});
