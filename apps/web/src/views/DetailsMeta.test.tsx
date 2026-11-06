import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Ship, Ticket } from "../api/types";
import { DetailsMeta } from "./DetailsMeta";

const card = (extra: Partial<Ticket>) => ({ key: "DEMO-3", ...extra }) as Ticket;
const ship = { path: "/work/ships/demo-3", branch: "ship/demo/three", key: "DEMO-3", ticket_id: "t" } as Ship;

describe("DetailsMeta", () => {
  it("shows the out-of-order mark and the ship", () => {
    const html = renderToStaticMarkup(
      <DetailsMeta card={card({ done_out_of_order: { behind: ["DEMO-1"] } })} ship={ship} />,
    );
    expect(html).toContain("done out of order: DEMO-1 still open");
    expect(html).toContain("/work/ships/demo-3");
    expect(html).toContain("ship/demo/three");
  });
  it("renders nothing for a plain card without a ship", () => {
    expect(renderToStaticMarkup(<DetailsMeta card={card({})} ship={null} />)).toBe("");
  });
});
