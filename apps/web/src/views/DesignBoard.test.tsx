import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ChainDetail, Design, Ticket } from "../api/types";
import type { Item } from "../store/designBoard";
import type { Pin } from "../store/pins";
import type { State } from "../store/state";
import { BoardTile } from "./BoardTile";
import { DesignBoard, EMPTY_HINT } from "./DesignBoard";

const design = (over: Partial<Design> = {}): Design => ({
  id: "D1", name: "login.png", kind: "image", mime: "image/png", size: 10, created_at: 0, figma: null, has_file: true, thumb: true, hint: null, ...over,
});
const region = { x: 20, y: 10, w: 40, h: 30 };
const item = (over: Partial<Item> = {}): Item => ({
  id: "D1", name: "login.png", design: design(),
  uses: [
    { card: "T-1", ticket: "i1", status: "running", region },
    { card: "T-2", ticket: "i2", status: "merged", region: null },
  ],
  ...over,
});
const pin = { id: "P1", number: 2, design: "D1", region, text: "Which colour?", answer: null } as unknown as Pin;
const none = () => {};
const tile = (over: Partial<Parameters<typeof BoardTile>[0]> = {}) =>
  renderToStaticMarkup(
    <BoardTile
      project="DEMO" item={item()} tile={{ id: "D1", x: 32, y: 32, w: 520, h: 390 }} pins={[pin]} activePin={undefined} onPin={none}
      onAspect={none} current={undefined} drag={undefined} actions={{ open: none, grab: none }} {...over}
    />,
  );

beforeEach(() => {
  vi.stubGlobal("window", { location: { search: "?token=t0k" } });
});

describe("a design on the board", () => {
  it("draws the picture from the project's file and outlines every card's region in its status colour", () => {
    const html = tile();
    expect(html).toContain("/v1/projects/DEMO/designs/D1/file?token=t0k");
    expect(html).toMatch(/class="board-region s-running[^"]*"[^>]*data-card="T-1"[^>]*style="left:20%;top:10%;width:40%;height:30%"/);
    // A card without a region has the whole picture.
    expect(html).toMatch(/class="board-region s-merged[^"]*"[^>]*data-card="T-2"[^>]*style="left:0%;top:0%;width:100%;height:100%"/);
    expect(html.match(/data-testid="board-label"/g)).toHaveLength(2);
    expect(html.match(/data-testid="board-handle"/g)).toHaveLength(2);
  });

  it("shows the pins as numbered markers on the picture", () => {
    const html = tile();
    expect(html).toContain('data-testid="pin-marker"');
    expect(html).toContain('data-number="2"');
    expect(tile({ pins: [] })).not.toContain("pin-marker");
  });

  it("sets labels that start at the same corner side by side", () => {
    const two = item({ uses: [
      { card: "T-1", ticket: "i1", status: "ready", region: null },
      { card: "T-2", ticket: "i2", status: "ready", region: null },
    ] });
    const html = tile({ item: two });
    expect(html).toMatch(/class="board-tag" style="left:0"/);
    expect(html).toContain("left:78px");
  });

  it("marks the open card's region and the region a handle is over", () => {
    expect(tile({ current: "T-1" })).toMatch(/board-region s-running current[^>]*data-card="T-1"/);
    const drag = { card: "T-2", ref: { design: "D1" }, from: { x: 0, y: 0 }, at: { x: 1, y: 1 }, over: "T-1" };
    expect(tile({ drag })).toMatch(/board-region s-running over[^>]*data-card="T-1"/);
  });

  it("is a note with the cards for a pdf, a Figma link without an image, and a design the library lost", () => {
    const pdf = design({ kind: "pdf", thumb: false, mime: "application/pdf" });
    const html = tile({ item: item({ design: pdf }) });
    expect(html).toContain('data-testid="board-note"');
    expect(html).not.toContain('data-testid="board-image"');
    expect(html.match(/data-testid="board-note-card"/g)).toHaveLength(2);
    expect(tile({ item: item({ design: undefined }) })).toContain("no longer in the project&#x27;s library");
    const figma = design({ kind: "figma", thumb: false, has_file: false, hint: "Export the frame from Figma." });
    expect(tile({ item: item({ design: figma }) })).toContain("Export the frame from Figma.");
  });
});

describe("the Designs view of a chain without designs", () => {
  const ticket = (over: Partial<Ticket>) => ({ id: "i", key: "T-1", status: "ready", designs: [], ...over }) as Ticket;
  const board = (tickets: Ticket[]) =>
    renderToStaticMarkup(
      <DesignBoard chain={{ chain: { id: "c", name: "work" }, tickets } as unknown as ChainDetail} state={{} as State} dispatch={none} />,
    );

  it("says there are none, with the hint", () => {
    for (const html of [board([]), board([ticket({})]), board([ticket({ designs: undefined })])]) {
      expect(html).toContain('data-testid="designs-empty"');
      expect(html).toContain(EMPTY_HINT);
      expect(html).not.toContain("design-board");
    }
  });

  it("a cancelled card's design does not make a board", () => {
    expect(board([ticket({ status: "cancelled", designs: [{ design: "D1" }] })])).toContain('data-testid="designs-empty"');
  });
});
