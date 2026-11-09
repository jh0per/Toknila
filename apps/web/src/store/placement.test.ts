import { afterEach, describe, expect, it, vi } from "vitest";
import type { ChainDetail } from "../api/types";
import type { Msg } from "./msg";
import { reduce } from "./reduce";
import { type State, initialState } from "./state";
import { runPlacementEffect } from "./placementEffects";

const chain = (pos?: { x: number; y: number }): ChainDetail =>
  ({
    chain: { id: "c1", name: "plan", goal: "", autostart: false, project_id: "p1" },
    tickets: [{ id: "t1", key: "DEMO-1", ...(pos ? { pos } : {}) }, { id: "t2", key: "DEMO-2" }],
    releases: [], edges: [], release_tickets: [], order: [],
    summary: { status: "not_started", total: 0, done: 0, released: 0 },
  }) as unknown as ChainDetail;

const start = (): State => reduce(initialState({ name: "chains" }), { type: "chainArranged", chain: chain() })[0];
const run = (s: State, ...msgs: Msg[]): State => msgs.reduce((at, m) => reduce(at, m)[0], s);
const place = (x: number, y: number): Msg => ({ type: "placeCard", chain: "c1", ticket: "t1", key: "DEMO-1", pos: { x, y } });

describe("card places in the store", () => {
  it("shows a moved card at its new place at once and asks the daemon to save it", () => {
    const [s, fx] = reduce(start(), place(248, 40));
    expect(s.placement.pending).toEqual({ t1: { x: 248, y: 40 } });
    expect(fx).toEqual([{ type: "placeCard", chain: "c1", ticket: "t1", key: "DEMO-1", pos: { x: 248, y: 40 } }]);
  });

  it("makes the place the card's own when the daemon confirms it", () => {
    const s = run(start(), place(248, 40), { type: "cardPlaced", chain: "c1", ticket: "t1", pos: { x: 248, y: 40 } });
    expect(s.placement.pending).toEqual({});
    expect(s.chains.c1.tickets[0].pos).toEqual({ x: 248, y: 40 });
  });

  it("lets a newer move win over an older answer", () => {
    const s = run(start(), place(248, 40), place(256, 40), { type: "cardPlaced", chain: "c1", ticket: "t1", pos: { x: 248, y: 40 } });
    expect(s.placement.pending).toEqual({ t1: { x: 256, y: 40 } });
    expect(s.chains.c1.tickets[0].pos).toBeUndefined();
  });

  it("takes the place back and says why when the daemon refuses", () => {
    const s = run(start(), place(248, 40), { type: "cardPlaceRefused", ticket: "t1", message: "no such card" });
    expect(s.placement.pending).toEqual({});
    expect(s.edges.error).toBe("no such card");
  });

  it("drops every place of the chain when it is arranged", () => {
    const s = run(start(), place(248, 40), { type: "chainArranged", chain: chain() });
    expect(s.placement.pending).toEqual({});
    expect(s.chains.c1.tickets[0].pos).toBeUndefined();
  });
});

describe("card sizes in the store", () => {
  const size = (w: number, h: number): Msg => ({ type: "sizeCard", chain: "c1", ticket: "t1", key: "DEMO-1", size: { w, h } });

  it("shows a resized card at its new size at once and asks the daemon to save it", () => {
    const [s, fx] = reduce(start(), size(280, 120));
    expect(s.placement.sizes).toEqual({ t1: { w: 280, h: 120 } });
    expect(fx).toEqual([{ type: "sizeCard", chain: "c1", ticket: "t1", key: "DEMO-1", size: { w: 280, h: 120 } }]);
  });

  it("makes the size the card's own when confirmed, and null takes it away", () => {
    const sized = run(start(), size(280, 120), { type: "cardSized", chain: "c1", ticket: "t1", size: { w: 280, h: 120 } });
    expect(sized.placement.sizes).toEqual({});
    expect(sized.chains.c1.tickets[0].size).toEqual({ w: 280, h: 120 });
    const cleared = run(sized, { type: "sizeCard", chain: "c1", ticket: "t1", key: "DEMO-1", size: null }, { type: "cardSized", chain: "c1", ticket: "t1", size: null });
    expect(cleared.chains.c1.tickets[0].size).toBeUndefined();
  });

  it("lets a newer resize win, and a refusal takes the size back", () => {
    const s = run(start(), size(280, 120), size(300, 120), { type: "cardSized", chain: "c1", ticket: "t1", size: { w: 280, h: 120 } });
    expect(s.placement.sizes).toEqual({ t1: { w: 300, h: 120 } });
    const refused = run(s, { type: "cardPlaceRefused", ticket: "t1", message: "no" });
    expect(refused.placement.sizes).toEqual({});
  });

  it("drops the sizes of the chain when it is arranged", () => {
    const s = run(start(), size(280, 120), { type: "chainArranged", chain: chain() });
    expect(s.placement.sizes).toEqual({});
    expect(s.chains.c1.tickets[0].size).toBeUndefined();
  });

  it("patches the card with the size", async () => {
    vi.stubGlobal("window", { location: { search: "?token=t" } });
    const calls: { url: string; init: RequestInit }[] = [];
    vi.stubGlobal("fetch", (url: string, init: RequestInit) => {
      calls.push({ url, init });
      return Promise.resolve(new Response("{}", { status: 200 }));
    });
    const got: Msg[] = [];
    runPlacementEffect({ type: "sizeCard", chain: "c1", ticket: "t1", key: "DEMO-1", size: null }, (m) => got.push(m));
    await vi.waitFor(() => expect(got).toHaveLength(1));
    expect(calls[0].init.method).toBe("PATCH");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ size: null });
    expect(got[0].type).toBe("cardSized");
  });
});

describe("card place requests", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("patches the card with the place, then arranges with a POST", async () => {
    vi.stubGlobal("window", { location: { search: "?token=t" } });
    const calls: { url: string; init: RequestInit }[] = [];
    vi.stubGlobal("fetch", (url: string, init: RequestInit) => {
      calls.push({ url, init });
      return Promise.resolve(new Response(JSON.stringify(chain()), { status: 200 }));
    });
    const got: Msg[] = [];
    runPlacementEffect({ type: "placeCard", chain: "c1", ticket: "t1", key: "DEMO-1", pos: { x: 8, y: 16 } }, (m) => got.push(m));
    runPlacementEffect({ type: "arrangeChain", chain: "c1" }, (m) => got.push(m));
    await vi.waitFor(() => expect(got).toHaveLength(2));
    expect(calls.map((c) => [c.init.method, c.url])).toEqual([["PATCH", "/v1/tickets/DEMO-1"], ["POST", "/v1/chains/c1/arrange"]]);
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ pos: { x: 8, y: 16 } });
    expect(got.map((m) => m.type)).toEqual(["cardPlaced", "chainArranged"]);
  });
});
