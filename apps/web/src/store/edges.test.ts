import { afterEach, describe, expect, it, vi } from "vitest";
import type { ChainDetail } from "../api/types";
import { runEdgeEffect } from "./edgeEffects";
import { type EdgeRef, idOf } from "./edges";
import type { Msg } from "./msg";
import { reduce } from "./reduce";
import { type State, initialState } from "./state";

const chain = (edges: number): ChainDetail =>
  ({
    chain: { id: "c1", name: "plan", goal: "", autostart: false, project_id: "p1" },
    tickets: [], releases: [], edges: Array.from({ length: edges }, () => ({})), release_tickets: [], order: [],
    summary: { status: "not_started", total: 0, done: 0, released: 0 },
  }) as unknown as ChainDetail;

const ab: EdgeRef = { chain: "c1", from: "ticket:A", to: "ticket:B" };
const start = (): State => initialState({ name: "chains" });
const run = (s: State, ...msgs: Msg[]): State => msgs.reduce((at, m) => reduce(at, m)[0], s);

describe("edge editing in the store", () => {
  it("asks the daemon and waits: nothing is guessed, the line is not added locally", () => {
    const [busy, fx] = reduce(start(), { type: "addEdge", chain: "c1", from: "ticket:A", to: "ticket:B" });
    expect(fx).toEqual([{ type: "addEdge", chain: "c1", from: "ticket:A", to: "ticket:B" }]);
    expect(busy.edges).toMatchObject({ busy: true, error: undefined });
    expect(busy.chains).toEqual({});
  });

  it("takes the daemon's answer as the chain and selects the line it made", () => {
    const s = run(start(), { type: "addEdge", chain: "c1", from: "ticket:A", to: "ticket:B" }, {
      type: "edgesChanged", chain: chain(1), selected: ab,
    });
    expect(s.chains.c1.edges).toHaveLength(1);
    expect(s.edges).toEqual({ selected: ab, busy: false, error: undefined });
  });

  it("shows a refusal word for word, keeps the selection and the chain", () => {
    const message = "dependency cycle: DEMO-1 -> DEMO-2 -> DEMO-1";
    const s0 = run(start(), { type: "selectEdge", edge: ab }, { type: "edgesChanged", chain: chain(2), selected: ab });
    const s = run(s0, { type: "removeEdge", chain: "c1", from: ab.from, to: ab.to }, { type: "edgeRefused", message });
    expect(s.edges).toEqual({ selected: ab, busy: false, error: message });
    expect(s.chains.c1.edges).toHaveLength(2);
    expect(run(s, { type: "dismissEdgeError" }).edges.error).toBeUndefined();
    expect(run(s, { type: "selectEdge", edge: undefined }).edges).toMatchObject({ selected: undefined, error: undefined });
  });

  it("reads the id out of a node key", () => {
    expect(idOf("release:01HX")).toBe("01HX");
  });
});

describe("edge requests", () => {
  afterEach(() => vi.unstubAllGlobals());

  const bodies: string[] = [];
  /// Answers each request in turn with the status given (200 by default): a chain, or an error body.
  function daemon(answers: number[]) {
    const calls: string[] = [];
    bodies.length = 0;
    vi.stubGlobal("window", { location: { search: "?token=t" } });
    vi.stubGlobal("fetch", async (url: string, init: RequestInit) => {
      calls.push(`${init.method} ${url}`);
      bodies.push(typeof init.body === "string" ? init.body : "");
      const status = answers[calls.length - 1] ?? 200;
      const body = status === 200 ? chain(calls.length) : { error: `refused ${calls.length}`, code: "conflict" };
      return new Response(JSON.stringify(body), { status });
    });
    return calls;
  }
  /// Runs the effect and waits for the one message it dispatches.
  const answer = (e: Parameters<typeof runEdgeEffect>[0]) =>
    new Promise<Msg>((resolve) => runEdgeEffect(e, resolve));
  const move = {
    type: "moveEdge", chain: "c1", from: "ticket:A", to: "ticket:B", kind: "normal", end: "to", target: "ticket:C",
  } as const;

  it("moves a line in one request with the old and the new line, and selects the new one", async () => {
    const calls = daemon([]);
    const msg = await answer(move);
    expect(calls).toEqual(["POST /v1/chains/c1/edges/move"]);
    expect(JSON.parse(bodies[0])).toEqual({
      old: { from: "A", to: "B" },
      new: { from: "A", to: "C", kind: "normal" },
    });
    expect(msg).toMatchObject({ type: "edgesChanged", selected: { chain: "c1", from: "ticket:A", to: "ticket:C" } });
  });

  it("is one request even when the daemon refuses it: nothing else is sent to undo", async () => {
    const calls = daemon([409]);
    expect(await answer(move)).toEqual({ type: "edgeRefused", message: "refused 1" });
    expect(calls).toEqual(["POST /v1/chains/c1/edges/move"]);
  });

  it("moves the start of a line by replacing its from", async () => {
    const calls = daemon([]);
    const msg = await answer({ ...move, end: "from", target: "ticket:D" });
    expect(calls).toEqual(["POST /v1/chains/c1/edges/move"]);
    expect(JSON.parse(bodies[0]).new).toEqual({ from: "D", to: "B", kind: "normal" });
    expect(msg).toMatchObject({ selected: { from: "ticket:D", to: "ticket:B" } });
  });

  it("removes a line by its two ends", async () => {
    const calls = daemon([]);
    const msg = await answer({ type: "removeEdge", chain: "c1", from: "ticket:A", to: "release:R" });
    expect(calls).toEqual(["DELETE /v1/chains/c1/edges?from=A&to=R"]);
    expect(msg).toMatchObject({ type: "edgesChanged", selected: undefined });
  });
});
