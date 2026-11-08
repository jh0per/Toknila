import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ChainDetail, Ticket } from "../api/types";
import { initialState } from "../store/state";
import { DesignFirst } from "./DesignFirst";
import { NewCard } from "./NewCard";
import { NoThreads } from "./ThreadsTab";

const card = (extra: Partial<Ticket> = {}) => ({ key: "DEMO-3", ...extra }) as Ticket;
const none = () => {};

describe("the Show me a design before the code choice", () => {
  it("is a checkbox with its words, off unless the card says so", () => {
    const off = renderToStaticMarkup(<DesignFirst checked={false} onChange={none} />);
    expect(off).toContain("Show me a design before the code");
    expect(off).toContain('data-testid="card-design-first"');
    expect(off).not.toContain("checked");
    expect(renderToStaticMarkup(<DesignFirst checked onChange={none} />)).toContain("checked");
  });

  it("is on the new card form, off by default", () => {
    const chain = { chain: { id: "c", name: "demo" }, tickets: [], order: [], edges: [], releases: [] } as unknown as ChainDetail;
    const html = renderToStaticMarkup(<NewCard chain={chain} state={initialState({ view: "home" } as never)} dispatch={none} />);
    expect(html).toContain("Show me a design before the code");
    expect(html).not.toContain("checked");
  });
});

describe("a card without threads", () => {
  it("offers Ask for a design with a line of why, when the daemon allows it", () => {
    const html = renderToStaticMarkup(<NoThreads card={card()} canAsk dispatch={none} />);
    expect(html).toContain('data-testid="ask-design"');
    expect(html).toContain("Ask for a design");
    expect(html).toContain("waits for your approval before it writes any code");
  });

  it("says a design is asked for once it is, without a button", () => {
    const html = renderToStaticMarkup(<NoThreads card={card({ design_first: true })} canAsk={false} dispatch={none} />);
    expect(html).toContain("A design is asked for");
    expect(html).not.toContain("<button");
  });

  it("offers nothing on a card that can no longer take it", () => {
    const html = renderToStaticMarkup(<NoThreads card={card()} canAsk={false} dispatch={none} />);
    expect(html).toContain("No threads on this card.");
    expect(html).not.toContain("<button");
  });
});
