import { describe, expect, it } from "vitest";
import { CORNER_ROOM, GAP_X, GAP_Y, GRID, H, PAD, W, canvasSize, columnXs, edgeGeometry, edgeId, inPoint, laneYs, outPoint, snapPos } from "./canvasGeometry";

const x = (col: number) => PAD + col * (W + GAP_X);
const y = (lane: number) => PAD + lane * (H + GAP_Y);
const node = (col: number, lane: number) => ({ x: x(col), y: y(lane), w: W, h: H });

describe("edgeGeometry", () => {
  it("keeps a single line at the middle of each card's edge", () => {
    const at = new Map([["a", node(0, 0)], ["b", node(1, 0)]]);
    const g = edgeGeometry([{ from: "a", to: "b" }], at).get(edgeId({ from: "a", to: "b" }));
    expect(g?.from).toEqual(outPoint(node(0, 0)));
    expect(g?.to).toEqual(inPoint(node(1, 0)));
  });

  it("starts and ends lines at the real edges of boxes of different sizes", () => {
    const a = { x: 16, y: 16, w: 320, h: 120 };
    const b = { x: 400, y: 200, w: 240, h: 56 };
    const g = edgeGeometry([{ from: "a", to: "b" }], new Map([["a", a], ["b", b]])).get("a>b");
    expect(g?.from).toEqual({ x: 336, y: 76 });
    expect(g?.to).toEqual({ x: 400, y: 228 });
  });

  it("spreads eight lines from one card over separate points, in the order of their targets", () => {
    const at = new Map<string, { x: number; y: number; w: number; h: number }>([["root", node(0, 4)]]);
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

  it("keeps the lines of a short card inside its own height", () => {
    const short = { x: 16, y: 100, w: 196, h: 56 };
    const at = new Map([["s", short], ["a", node(1, 0)], ["b", node(1, 1)], ["c", node(1, 2)]]);
    const edges = [{ from: "s", to: "a" }, { from: "s", to: "b" }, { from: "s", to: "c" }];
    const geo = edgeGeometry(edges, at);
    for (const e of edges) {
      const v = geo.get(edgeId(e))?.from.y ?? Number.NaN;
      expect(v).toBeGreaterThan(short.y);
      expect(v).toBeLessThan(short.y + short.h);
    }
  });

  it("spreads the lines that enter one card at separate points of its left edge", () => {
    const at = new Map<string, { x: number; y: number; w: number; h: number }>([["sink", node(1, 0)]]);
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
    expect(outPoint({ x: 100, y: 40, w: 250, h: 60 })).toEqual({ x: 350, y: 70 });
    expect(inPoint({ x: 100, y: 40, w: 250, h: 60 })).toEqual({ x: 100, y: 70 });
  });

  it("starts each column after the widest node of the one before, and each lane after the tallest", () => {
    expect(columnXs([196, 320, 196])).toEqual([PAD, PAD + 196 + GAP_X, PAD + 196 + GAP_X + 320 + GAP_X]);
    expect(laneYs([56, 96])).toEqual([PAD, PAD + 56 + GAP_Y]);
    expect(columnXs([W, W]).map((v, i) => v - PAD - i * (W + GAP_X))).toEqual([0, 0]);
  });

  it("snaps a dropped card to the grid and keeps it inside the padding", () => {
    expect(snapPos({ x: 251, y: 99 })).toEqual({ x: 248, y: 96 });
    expect(snapPos({ x: 252, y: 100 })).toEqual({ x: 256, y: 104 });
    expect(snapPos({ x: -90, y: 3 })).toEqual({ x: PAD, y: PAD });
    expect(GRID).toBe(8);
  });

  it("grows the canvas to fit the farthest box", () => {
    expect(canvasSize([])).toEqual({ width: 2 * PAD, height: 2 * PAD + CORNER_ROOM });
    expect(canvasSize([{ x: PAD, y: PAD, w: W, h: H }, { x: 600, y: 300, w: 320, h: 120 }])).toEqual({
      width: 600 + 320 + PAD, height: 300 + 120 + PAD + CORNER_ROOM,
    });
  });
});
