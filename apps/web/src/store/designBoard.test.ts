import { describe, expect, it } from "vitest";
import type { Design, Ticket } from "../api/types";
import {
  CAPTION, COLUMNS, DEFAULT_ASPECT, GAP, PAD, TILE_W, WHOLE, boardItems, dropTarget, isPicture, labelSlots, layoutBoard, refIndex,
  regionOf,
} from "./designBoard";

const design = (id: string, over: Partial<Design> = {}): Design => ({
  id, name: `${id}.png`, kind: "image", mime: "image/png", size: 10, created_at: 0, figma: null, has_file: true, thumb: true, hint: null, ...over,
});
const card = (key: string, designs: Ticket["designs"], status: Ticket["status"] = "ready"): Ticket =>
  ({ id: `id-${key}`, key, status, designs }) as Ticket;
const r1 = { x: 10, y: 20, w: 30, h: 40 };

describe("which designs the board shows", () => {
  const lib = [design("A"), design("B")];

  it("lists each design once with every card that uses it, in the order the cards name them", () => {
    const items = boardItems(
      [card("T-1", [{ design: "B", region: r1 }]), card("T-2", [{ design: "A" }, { design: "B" }]), card("T-3", undefined)],
      lib,
    );
    expect(items.map((i) => i.id)).toEqual(["B", "A"]);
    expect(items[0].uses).toEqual([
      { card: "T-1", ticket: "id-T-1", status: "ready", region: r1 },
      { card: "T-2", ticket: "id-T-2", status: "ready", region: null },
    ]);
    expect(items[0].name).toBe("B.png");
  });

  it("keeps a design the library lost, under its id, and leaves out a cancelled card's references", () => {
    const items = boardItems([card("T-1", [{ design: "GONE" }]), card("T-2", [{ design: "A" }], "cancelled")], lib);
    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({ id: "GONE", name: "GONE", design: undefined });
  });

  it("an empty chain has no items", () => {
    expect(boardItems([card("T-1", [])], lib)).toEqual([]);
    expect(boardItems([], lib)).toEqual([]);
  });

  it("only a kept file that can be a picture is drawn as one", () => {
    expect(isPicture(design("A"))).toBe(true);
    expect(isPicture(design("P", { kind: "pdf", thumb: false }))).toBe(false);
    // A pdf whose first page was drawn is a picture on the board.
    expect(isPicture(design("P", { kind: "pdf", thumb: true }))).toBe(true);
    expect(isPicture(design("F", { kind: "figma", thumb: false, has_file: false }))).toBe(false);
    expect(isPicture(undefined)).toBe(false);
  });
});

describe("the board layout", () => {
  it("has no size without tiles", () => {
    expect(layoutBoard([], {})).toEqual({ tiles: [], width: 0, height: 0 });
  });

  it("puts one tile in the padding corner at the default aspect", () => {
    const b = layoutBoard(["A"], {});
    expect(b.tiles).toEqual([{ id: "A", x: PAD, y: PAD, w: TILE_W, h: TILE_W / DEFAULT_ASPECT }]);
    expect(b.width).toBe(PAD * 2 + TILE_W);
    expect(b.height).toBe(PAD * 2 + TILE_W / DEFAULT_ASPECT + CAPTION);
  });

  it("fills a row left to right, then starts the next one under the tallest tile of the row", () => {
    const ids = ["A", "B", "C", "D"];
    const b = layoutBoard(ids, { A: 1, B: 2, C: 1, D: 1 });
    expect(COLUMNS).toBe(3);
    expect(b.tiles.map((t) => [t.x, t.y])).toEqual([
      [PAD, PAD], [PAD + TILE_W + GAP, PAD], [PAD + 2 * (TILE_W + GAP), PAD], [PAD, PAD + TILE_W + CAPTION + GAP],
    ]);
    expect(b.tiles[1].h).toBe(TILE_W / 2);
    expect(b.width).toBe(PAD * 2 + 3 * TILE_W + 2 * GAP);
    expect(b.height).toBe(PAD * 2 + TILE_W + CAPTION + GAP + TILE_W + CAPTION);
  });

  it("clamps a thin or huge aspect and ignores one that is not a number", () => {
    const b = layoutBoard(["A", "B", "C"], { A: 100, B: 0.001, C: Number.NaN });
    expect(b.tiles.map((t) => t.h)).toEqual([TILE_W / 4, TILE_W / 0.25, TILE_W / DEFAULT_ASPECT]);
  });
});

describe("where a label sits", () => {
  it("shifts uses that start at the same corner and leaves the others", () => {
    const use = (region: typeof r1 | null) => ({ card: "T", ticket: "i", status: "ready" as const, region });
    expect(labelSlots([use(null), use(r1), use(null), use({ ...WHOLE }), use(r1)])).toEqual([0, 0, 1, 2, 1]);
  });

  it("a use without a region is the whole picture", () => {
    expect(regionOf({ card: "T", ticket: "i", status: "ready", region: null })).toEqual(WHOLE);
  });
});

describe("the card a handle is dropped on", () => {
  const boxes = [
    { card: "T-1", x: 0, y: 0, w: 200, h: 200 },
    { card: "T-2", x: 50, y: 50, w: 40, h: 20 },
    { card: "T-3", x: 300, y: 0, w: 50, h: 50 },
  ];

  it("takes the smallest box that holds the point", () => {
    expect(dropTarget(boxes, { x: 60, y: 60 }, "T-9")).toBe("T-2");
    expect(dropTarget(boxes, { x: 10, y: 10 }, "T-9")).toBe("T-1");
    expect(dropTarget(boxes, { x: 310, y: 10 }, "T-9")).toBe("T-3");
  });

  it("is nothing for a drop on empty board, and never the card the handle came from", () => {
    expect(dropTarget(boxes, { x: 250, y: 250 }, "T-9")).toBeUndefined();
    expect(dropTarget(boxes, { x: 10, y: 10 }, "T-1")).toBeUndefined();
    // Inside the source's own small box, the bigger box of another card still counts.
    expect(dropTarget(boxes, { x: 60, y: 60 }, "T-2")).toBe("T-1");
  });
});

describe("finding the reference to move", () => {
  it("is its index in the card's list, the number the move route takes", () => {
    expect(refIndex([{ design: "A", region: r1 }, { design: "B" }], { design: "B" })).toBe(1);
  });

  it("matches the whole-picture reference written as a null region", () => {
    expect(refIndex([{ design: "A" }], { design: "A", region: null })).toBe(0);
  });

  it("picks only the one with that region", () => {
    const other = { x: 1, y: 1, w: 5, h: 5 };
    expect(refIndex([{ design: "A", region: r1 }, { design: "A", region: other }], { design: "A", region: other })).toBe(1);
  });

  it("is nothing when the card does not hold the reference", () => {
    expect(refIndex([{ design: "B" }], { design: "A" })).toBeUndefined();
    expect(refIndex([{ design: "A", region: r1 }], { design: "A" })).toBeUndefined();
  });
});
