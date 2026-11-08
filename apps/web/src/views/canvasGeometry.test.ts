import { describe, expect, it } from "vitest";
import { GAP_X, GAP_Y, GRID, H, PAD, W, canvasSize, edgeGeometry, edgeId, inPoint, outPoint, snapPos, x, y } from "./canvasGeometry";

const node = (col: number, lane: number) => ({ x: x(col), y: y(lane) });

describe("edgeGeometry", () => {
  it("keeps a single line at the middle of each card's edge", () => {
    const at = new Map([["a", node(0, 0)], ["b", node(1, 0)]]);
    const g = edgeGeometry([{ from: "a", to: "b" }], at).get(edgeId({ from: "a", to: "b" }));
    expect(g?.from).toEqual(outPoint(node(0, 0)));
    expect(g?.to).toEqual(inPoint(node(1, 0)));
  });

  it("spreads eight lines from one card over separate points, in the order of their targets", () => {
    const at = new Map<string, { x: number; y: number }>([["root", node(0, 4)]]);
    const edges = [];
    for (let i = 0; i < 8; i++) {
      at.set(`t${i}`, node(1, i));
      edges.push({ from: "root", to: `t${i}` });
    }
    const geo = edgeGeometry(edges, at);
    const ys = edges.map((e) => geo.get(edgeId(e))?.from.y ?? Number.NaN);
    expect(new Set(ys).size).toBe(8);
    expect([...ys].sort((a, b) => a - b)).toEqual(ys);
    // Every start stays inside the card.
    for (const v of ys) {
      expect(v).toBeGreaterThan(y(4));
      expect(v).toBeLessThan(y(4) + H);
    }
    // And their curves turn at separate places, so they do not share one vertical trunk.
    const turns = edges.map((e) => geo.get(edgeId(e))?.turn ?? Number.NaN);
    expect(new Set(turns).size).toBe(8);
  });

  it("spreads the lines that enter one card at separate points of its left edge", () => {
    const at = new Map<string, { x: number; y: number }>([["sink", node(1, 0)]]);
    const edges = [];
    for (let i = 0; i < 3; i++) {
      at.set(`s${i}`, node(0, i));
      edges.push({ from: `s${i}`, to: "sink" });
    }
    const geo = edgeGeometry(edges, at);
    const ys = edges.map((e) => geo.get(edgeId(e))?.to.y);
    expect(new Set(ys).size).toBe(3);
  });

  it("is the same for the same input", () => {
    const at = new Map([["a", node(0, 0)], ["b", node(1, 0)], ["c", node(1, 1)]]);
    const edges = [{ from: "a", to: "b" }, { from: "a", to: "c" }];
    expect(edgeGeometry(edges, at)).toEqual(edgeGeometry(edges, at));
  });
});

describe("pixels", () => {
  it("puts the connectors on the middle of the right and left edges of a box", () => {
    expect(outPoint({ x: 100, y: 40 })).toEqual({ x: 100 + W, y: 40 + H / 2 });
    expect(inPoint({ x: 100, y: 40 })).toEqual({ x: 100, y: 40 + H / 2 });
  });

  it("lays the auto grid out on the snap grid, so a card that was not moved is already on it", () => {
    for (const v of [x(0), x(3), y(0), y(5), x(1) - x(0), y(1) - y(0)]) expect(v % GRID).toBe(0);
    expect(W + GAP_X).toBe(240);
    expect(H + GAP_Y).toBe(112);
  });

  it("snaps a dropped card to the grid and keeps it inside the padding", () => {
    expect(snapPos({ x: 251, y: 99 })).toEqual({ x: 248, y: 96 });
    expect(snapPos({ x: 252, y: 100 })).toEqual({ x: 256, y: 104 });
    expect(snapPos({ x: -90, y: 3 })).toEqual({ x: PAD, y: PAD });
  });

  it("grows the canvas to fit the farthest box", () => {
    expect(canvasSize([])).toEqual({ width: 2 * PAD, height: 2 * PAD });
    expect(canvasSize([{ x: PAD, y: PAD }, { x: 600, y: 300 }])).toEqual({ width: 600 + W + PAD, height: 300 + H + PAD });
  });
});
