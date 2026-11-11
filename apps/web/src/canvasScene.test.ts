import { describe, expect, it } from "vitest";
import type { ChainDetail, NodeRef } from "./api/types";
import { FINISH, START, bookendLines, sceneOf } from "./canvasScene";
import { GAP_X, GAP_Y, H, PAD, W } from "./views/canvasGeometry";

const x = (col: number) => PAD + col * (W + GAP_X);

const t = (id: string): NodeRef => ({ kind: "ticket", id });
const r = (id: string): NodeRef => ({ kind: "release", id });

function chain(order: NodeRef[], edges: [NodeRef, NodeRef][]): ChainDetail {
  return {
    chain: { id: "c", name: "demo", goal: "ship it", autostart: true, project_id: "p" },
    tickets: order.filter((n) => n.kind === "ticket").map((n) => ({ id: n.id })),
    releases: order.filter((n) => n.kind === "release").map((n) => ({ id: n.id })),
    edges: edges.map(([from, to]) => ({ chain_id: "c", from, to })),
    release_tickets: [],
    order,
    summary: { status: "running", phase: "idle", finished_at: null, total: 0, done: 0, released: 0 },
  } as unknown as ChainDetail;
}

const none = () => undefined;
const implicit = (s: ReturnType<typeof sceneOf>) => s.lines.filter((l) => l.implicit).map((l) => `${l.from}>${l.to}`);

describe("bookendLines", () => {
  it("joins every card with nothing before it to Start and every card with nothing after it to the end", () => {
    const edges = [{ from: "a", to: "b" }, { from: "a", to: "c" }];
    const lines = bookendLines(["a", "b", "c", "d"], edges, FINISH);
    expect(lines.filter((l) => !l.implicit)).toEqual([
      { from: "a", to: "b", implicit: false },
      { from: "a", to: "c", implicit: false },
    ]);
    expect(lines.filter((l) => l.implicit).map((l) => `${l.from}>${l.to}`)).toEqual([
      `${START}>a`, `${START}>d`, `b>${FINISH}`, `c>${FINISH}`, `d>${FINISH}`,
    ]);
  });

  it("gives an empty chain a line from Start to the end", () => {
    expect(bookendLines([], [], FINISH)).toEqual([{ from: START, to: FINISH, implicit: true }]);
  });

  it("does not draw a line out of the end node, or a second one into a release that is the end", () => {
    const lines = bookendLines(["a", "R"], [{ from: "a", to: "R" }], "R");
    expect(lines.filter((l) => l.implicit).map((l) => `${l.from}>${l.to}`)).toEqual([`${START}>a`]);
  });
});

describe("sceneOf", () => {
  it("puts Start in column 0, cards from column 1 and Finish after the last card", () => {
    const s = sceneOf(chain([t("A"), t("B")], [[t("A"), t("B")]]), none);
    const col = Object.fromEntries(s.nodes.map((n) => [n.id === "demo" ? n.kind : n.id, n.col]));
    expect(col).toEqual({ start: 0, A: 1, B: 2, finish: 3 });
    expect(implicit(s)).toEqual([`${START}>ticket:A`, `ticket:B>${FINISH}`]);
  });

  it("makes the chain's last release its end node, in the last column", () => {
    const s = sceneOf(chain([t("A"), t("B"), r("R")], [[t("A"), r("R")]]), none);
    const end = s.nodes[s.nodes.length - 1];
    expect(end.key).toBe("release:R");
    expect(end.kind).toBe("release");
    expect(end.col).toBe(2);
    expect(s.nodes.some((n) => n.kind === "finish")).toBe(false);
    // B has no line into R, so the implicit one ends there; A's real line is not doubled.
    expect(implicit(s)).toEqual([`${START}>ticket:A`, `${START}>ticket:B`, "ticket:B>release:R"]);
  });

  it("keeps a release that has cards after it in the graph and ends at Finish", () => {
    const s = sceneOf(chain([t("A"), r("R"), t("C")], [[t("A"), r("R")], [r("R"), t("C")]]), none);
    expect(s.nodes.find((n) => n.key === "release:R")?.col).toBe(2);
    expect(implicit(s)).toEqual([`${START}>ticket:A`, `ticket:C>${FINISH}`]);
  });

  it("draws a card at its saved place and the others by the layout, and never moves the bookends", () => {
    const own = { x: 600, y: 304 };
    const s = sceneOf(chain([t("A"), t("B")], []), (id) => (id === "B" ? own : undefined));
    const a = s.nodes.find((n) => n.id === "A");
    const b = s.nodes.find((n) => n.id === "B");
    expect(b?.box).toEqual({ ...own, w: W, h: H });
    expect(b?.placed).toBe(true);
    expect(a?.box).toEqual({ x: x(1), y: PAD, w: W, h: H });
    expect(a?.placed).toBe(false);
    expect(s.nodes.find((n) => n.kind === "start")?.box.x).toBe(PAD);
    // The end node follows the automatic columns, not where a card was dropped.
    expect(s.nodes.find((n) => n.kind === "finish")?.box.x).toBe(x(2));
    expect(x(1)).toBe(PAD + W + GAP_X);
  });

  it("makes each column as wide as its widest card and each lane as tall as its tallest", () => {
    const sizes: Record<string, { w: number; h: number; own: boolean }> = {
      A: { w: 320, h: 72, own: false },
      B: { w: 240, h: 56, own: false },
      C: { w: 196, h: 96, own: false },
    };
    // A and B are both roots (column 1, lanes 0 and 1); C waits for A (column 2, lane 0).
    const s = sceneOf(chain([t("A"), t("B"), t("C")], [[t("A"), t("C")]]), none, (id) => sizes[id]);
    const box = (id: string) => s.nodes.find((n) => n.id === id)?.box;
    expect(box("A")).toEqual({ x: PAD + W + GAP_X, y: PAD, w: 320, h: 72 });
    // Lane 0 is as tall as C (96), so B starts below it.
    expect(box("B")).toEqual({ x: PAD + W + GAP_X, y: PAD + 96 + GAP_Y, w: 240, h: 56 });
    // Column 1 is 320 wide (A), so column 2 starts after it.
    expect(box("C")?.x).toBe(PAD + W + GAP_X + 320 + GAP_X);
    // Finish follows the widest card of the last column.
    expect(s.nodes.find((n) => n.kind === "finish")?.box.x).toBe(PAD + W + GAP_X + 320 + GAP_X + 196 + GAP_X);
  });

  it("centres Start and Finish on the height of the lanes", () => {
    const s = sceneOf(chain([t("A"), t("B")], []), none, () => ({ w: W, h: 200, own: false }));
    // Two lanes of 200 and a gap: 420 tall; a 92 px node sits in the middle.
    expect(s.nodes[0].box.y).toBe(PAD + Math.round((2 * 200 + GAP_Y - H) / 2));
  });

  it("does not move other cards when one is dragged away, and marks sized cards", () => {
    const sizes: Record<string, { w: number; h: number; own: boolean }> = { A: { w: 300, h: 72, own: true } };
    const base = sceneOf(chain([t("A"), t("B")], []), none, (id) => sizes[id]);
    const moved = sceneOf(chain([t("A"), t("B")], []), (id) => (id === "A" ? { x: 900, y: 500 } : undefined), (id) => sizes[id]);
    expect(moved.nodes.find((n) => n.id === "B")?.box).toEqual(base.nodes.find((n) => n.id === "B")?.box);
    expect(moved.nodes.find((n) => n.id === "A")?.box).toEqual({ x: 900, y: 500, w: 300, h: 72 });
    expect(base.nodes.find((n) => n.id === "A")?.sized).toBe(true);
    expect(base.nodes.find((n) => n.id === "B")?.sized).toBe(false);
  });
});
