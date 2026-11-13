import { afterEach, describe, expect, it, vi } from "vitest";
import type { ChainDetail, TicketDetail } from "../api/types";
import type { Msg } from "./msg";
import { reduce } from "./reduce";
import { type State, initialState } from "./state";
import { runPlacementEffect } from "./placementEffects";

const chain = (id: string, name: string, keys: string[]): ChainDetail =>
  ({
    chain: { id, name, goal: "", autostart: false, project_id: "p1" },
    tickets: keys.map((key) => ({ id: `t-${key}`, key, chain_id: id })),
    releases: [], edges: [], release_tickets: [], order: [],
    summary: { status: "not_started", total: 0, done: 0, released: 0 },
  }) as unknown as ChainDetail;

const moved = (key: string, to: string): TicketDetail =>
  ({ ticket: { id: `t-${key}`, key, chain_id: to }, chain: "android" }) as unknown as TicketDetail;

const start = (): State => reduce(initialState({ name: "chains" }), { type: "loaded", projects: [], chains: [chain("c1", "soundpacks", ["B-16"]), chain("c2", "android", [])] })[0];
const move: Msg = { type: "moveToEpic", key: "B-16", from: "c1", chain: "c2" };

describe("a card moving to another epic, in the store", () => {
  it("asks the daemon, and the drop target stops showing", () => {
    const over = reduce(start(), { type: "overEpic", chain: "c2" })[0];
    expect(over.placement.dropEpic).toBe("c2");
    const [s, fx] = reduce(over, move);
    expect(s.placement.dropEpic).toBeUndefined();
    expect(fx).toEqual([move]);
  });

  it("reads both epics again when the daemon has moved the card, and keeps its fresh detail", () => {
    const [s, fx] = reduce(start(), { type: "movedToEpic", from: "c1", detail: moved("B-16", "c2") });
    expect(fx).toEqual([{ type: "loadChain", id: "c1" }, { type: "loadChain", id: "c2" }]);
    expect(s.details["B-16"].ticket.chain_id).toBe("c2");
  });

  it("shows a refusal the way the diagram shows the others", () => {
    const s = reduce(start(), { type: "edgeRefused", message: "BEATSTER-16 is running" })[0];
    expect(s.edges.error).toBe("BEATSTER-16 is running");
  });

  it("reads the epic a card left when the journal says it moved", () => {
    const s = start();
    const payload = { from: "soundpacks", to: "android", ticket: { id: "t-B-16", key: "B-16", chain_id: "c2" } };
    const [, fx] = reduce(s, { type: "event", event: { seq: s.lastSeq + 1, at: 1, actor: "human", entity: "ticket", entity_id: "t-B-16", kind: "moved", payload } as never });
    expect(fx).toContainEqual({ type: "loadChain", id: "c2" });
    expect(fx).toContainEqual({ type: "loadChain", id: "c1" });
  });
});

describe("the move request", () => {
  afterEach(() => vi.unstubAllGlobals());
  const run = async (e: Parameters<typeof runPlacementEffect>[0], answer: () => Response) => {
    vi.stubGlobal("window", { location: { search: "?token=t" } });
    const calls: { url: string; init: RequestInit }[] = [];
    vi.stubGlobal("fetch", (url: string, init: RequestInit) => {
      calls.push({ url, init });
      return Promise.resolve(answer());
    });
    const got: Msg[] = [];
    runPlacementEffect(e, (m) => got.push(m));
    await vi.waitFor(() => expect(got).toHaveLength(1));
    return { calls, got };
  };

  it("posts the epic, and the place when there is one", async () => {
    const ok = () => new Response(JSON.stringify(moved("B-16", "c2")), { status: 200 });
    const end = await run({ type: "moveToEpic", key: "B-16", from: "c1", chain: "c2" }, ok);
    expect(end.calls[0].url).toBe("/v1/tickets/B-16/move-to-chain");
    expect(JSON.parse(String(end.calls[0].init.body))).toEqual({ chain: "c2" });
    expect(end.got[0].type).toBe("movedToEpic");
    const placed = await run({ type: "moveToEpic", key: "B-16", from: "c1", chain: "c2", place: { anchor: "B-3", place: "after" } }, ok);
    expect(JSON.parse(String(placed.calls[0].init.body))).toEqual({ chain: "c2", place: { anchor: "B-3", place: "after" } });
  });

  it("turns the daemon's refusal into the diagram's error, in the daemon's words", async () => {
    const no = () => new Response(JSON.stringify({ code: "conflict", error: "B-16 is running; it cannot move" }), { status: 409 });
    const { got } = await run(move as never, no);
    expect(got).toEqual([{ type: "edgeRefused", message: "B-16 is running; it cannot move" }]);
  });
});
