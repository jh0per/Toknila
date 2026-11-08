import { describe, expect, it } from "vitest";
import type { ChainDetail, NodeRef } from "./api/types";
import { FINISH, START, bookendLines, sceneOf } from "./canvasScene";
import { GAP_X, PAD, W, x } from "./views/canvasGeometry";

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
    summary: { status: "running", total: 0, done: 0, released: 0 },
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
    expect(b?.box).toEqual(own);
    expect(b?.placed).toBe(true);
    expect(a?.box).toEqual({ x: x(1), y: PAD });
    expect(a?.placed).toBe(false);
    expect(s.nodes.find((n) => n.kind === "start")?.box.x).toBe(PAD);
    // The end node follows the automatic columns, not where a card was dropped.
    expect(s.nodes.find((n) => n.kind === "finish")?.box.x).toBe(x(2));
    expect(x(1)).toBe(PAD + W + GAP_X);
  });
});
