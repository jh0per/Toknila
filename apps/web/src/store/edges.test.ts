import { afterEach, describe, expect, it, vi } from "vitest";
import type { ChainDetail } from "../api/types";
import { runEdgeEffect } from "./edgeEffects";
import { type EdgeRef, idOf, sameEdge } from "./edges";
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

  it("compares lines by chain and both ends", () => {
    expect(sameEdge(ab, { ...ab })).toBe(true);
    expect(sameEdge(ab, { ...ab, to: "ticket:C" })).toBe(false);
    expect(sameEdge(undefined, ab)).toBe(false);
    expect(idOf("release:01HX")).toBe("01HX");
  });
});

describe("edge requests", () => {
  afterEach(() => vi.unstubAllGlobals());

  /// Answers each request in turn with the status given (200 by default): a chain, or an error body.
  function daemon(answers: number[]) {
    const calls: string[] = [];
    vi.stubGlobal("window", { location: { search: "?token=t" } });
    vi.stubGlobal("fetch", async (url: string, init: RequestInit) => {
      calls.push(`${init.method} ${url}`);
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

  it("adds at the new place, then removes the old one, and selects the new line", async () => {
    const calls = daemon([]);
    const msg = await answer(move);
    expect(calls).toEqual(["POST /v1/chains/c1/edges", "DELETE /v1/chains/c1/edges?from=A&to=B"]);
    expect(msg).toMatchObject({ type: "edgesChanged", selected: { chain: "c1", from: "ticket:A", to: "ticket:C" } });
  });

  it("changes nothing when the new place is refused", async () => {
    const calls = daemon([409]);
    expect(await answer(move)).toEqual({ type: "edgeRefused", message: "refused 1" });
    expect(calls).toHaveLength(1);
  });

  it("takes the new line back when the old one cannot be removed", async () => {
    const calls = daemon([200, 409, 200]);
    expect(await answer(move)).toEqual({ type: "edgeRefused", message: "refused 2" });
    expect(calls).toEqual([
      "POST /v1/chains/c1/edges",
      "DELETE /v1/chains/c1/edges?from=A&to=B",
      "DELETE /v1/chains/c1/edges?from=A&to=C",
    ]);
  });

  it("moves the start of a line by replacing its from", async () => {
    const calls = daemon([]);
    const msg = await answer({ ...move, end: "from", target: "ticket:D" });
    expect(calls[1]).toBe("DELETE /v1/chains/c1/edges?from=A&to=B");
    expect(msg).toMatchObject({ selected: { from: "ticket:D", to: "ticket:B" } });
  });

  it("removes a line by its two ends", async () => {
    const calls = daemon([]);
    const msg = await answer({ type: "removeEdge", chain: "c1", from: "ticket:A", to: "release:R" });
    expect(calls).toEqual(["DELETE /v1/chains/c1/edges?from=A&to=R"]);
    expect(msg).toMatchObject({ type: "edgesChanged", selected: undefined });
  });
});
