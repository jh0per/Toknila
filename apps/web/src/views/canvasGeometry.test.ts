import { describe, expect, it } from "vitest";
import { H, edgeGeometry, edgeId, inPoint, outPoint, y } from "./canvasGeometry";

const node = (col: number, lane: number) => ({ col, lane });

describe("edgeGeometry", () => {
  it("keeps a single line at the middle of each card's edge", () => {
    const at = new Map([["a", node(0, 0)], ["b", node(1, 0)]]);
    const g = edgeGeometry([{ from: "a", to: "b" }], at).get(edgeId({ from: "a", to: "b" }));
    expect(g?.from).toEqual(outPoint(node(0, 0)));
    expect(g?.to).toEqual(inPoint(node(1, 0)));
  });

  it("spreads eight lines from one card over separate points, in the order of their targets", () => {
    const at = new Map<string, { col: number; lane: number }>([["root", node(0, 4)]]);
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
    const at = new Map<string, { col: number; lane: number }>([["sink", node(1, 0)]]);
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
