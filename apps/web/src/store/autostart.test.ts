import { describe, expect, it } from "vitest";
import type { ChainDetail } from "../api/types";
import { reduce } from "./reduce";
import { initialState } from "./state";

const chain = (autostart: boolean): ChainDetail =>
  ({
    chain: { id: "c1", name: "work", goal: "", autostart, project_id: "p1" },
    tickets: [], releases: [], edges: [], release_tickets: [], order: [],
    summary: { status: "not_started", total: 0, done: 0, released: 0 },
  }) as unknown as ChainDetail;

describe("the autostart toggle in the store", () => {
  it("asks the daemon and keeps the chain as it is until it answers", () => {
    const s0 = reduce(initialState({ name: "chains" }), { type: "autostartSet", chain: chain(false) })[0];
    const [s, fx] = reduce(s0, { type: "setAutostart", chain: "c1", autostart: true });
    expect(fx).toEqual([{ type: "setAutostart", chain: "c1", autostart: true }]);
    expect(s.chains.c1.chain.autostart).toBe(false);
  });

  it("takes the daemon's chain and reloads the agents' slots", () => {
    const [s, fx] = reduce(initialState({ name: "chains" }), { type: "autostartSet", chain: chain(true) });
    expect(s.chains.c1.chain.autostart).toBe(true);
    expect(fx).toEqual([{ type: "loadAgents" }]);
  });

  it("shows a refusal as the daemon said it", () => {
    const [s] = reduce(initialState({ name: "chains" }), { type: "autostartRefused", message: "no such chain" });
    expect(s.error).toBe("no such chain");
  });
});
