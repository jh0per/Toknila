import { describe, expect, it } from "vitest";
import type { ChainDetail } from "../api/types";
import { epicPanel } from "./epic";
import { reduce } from "./reduce";
import { initialState } from "./state";

const chain = (name: string, version?: string): ChainDetail =>
  ({
    chain: { id: "c1", name, goal: "g", autostart: true, project_id: "p1" },
    tickets: [], releases: [], edges: [], release_tickets: [], order: [],
    summary: { status: "idle", phase: "idle", finished_at: null, total: 0, done: 0, released: 0, releasable: false, version },
  }) as unknown as ChainDetail;

const route = { name: "projectChain" as const, project: "DEMO", chain: "auth" };
const base = () => ({ ...initialState(route), chains: { c1: chain("auth") }, chainOrder: ["c1"] });

describe("the epic and release panels", () => {
  it("opens for the page it was opened on and gives way to a card", () => {
    const [open] = reduce(base(), { type: "openEpic", chain: "c1", kind: "epic" });
    expect(epicPanel(open)?.kind).toBe("epic");
    const [card] = reduce(open, { type: "navigate", route: { name: "card", project: "DEMO", chain: "auth", key: "DEMO-1" } });
    expect(epicPanel(card)).toBeUndefined();
  });
  it("sends a save and shows the daemon's refusal as it said it", () => {
    const [open] = reduce(base(), { type: "openEpic", chain: "c1", kind: "release" });
    const save = { type: "saveEpic" as const, chain: "c1", project: "DEMO", from: "auth", patch: { version: "0.4.0" } };
    const [busy, fx] = reduce(open, save);
    expect(busy.epic.busy).toBe(true);
    expect(fx).toEqual([save]);
    const [refused] = reduce(busy, { type: "epicRefused", message: "release 0.3.0 is released: its version cannot change" });
    expect(refused.epic).toMatchObject({ busy: false, error: "release 0.3.0 is released: its version cannot change" });
  });
  it("takes the saved chain, and a rename moves the page and the open panel to the new name", () => {
    const [open] = reduce(base(), { type: "openEpic", chain: "c1", kind: "epic" });
    const [saved, fx] = reduce(open, { type: "epicSaved", project: "DEMO", from: "auth", chain: chain("login", "0.3.0") });
    expect(saved.chains.c1.summary.version).toBe("0.3.0");
    expect(saved.route).toEqual({ name: "projectChain", project: "DEMO", chain: "login" });
    expect(fx).toContainEqual({ type: "pushUrl", path: "/projects/DEMO/chains/login" });
    expect(epicPanel(saved)?.kind).toBe("epic");
  });
  it("asks for the version a new chain suggests when its dialog opens, and keeps the answer", () => {
    const [open, fx] = reduce(base(), { type: "openDialog", dialog: { kind: "newChain", project: "DEMO" } });
    expect(fx).toContainEqual({ type: "loadNextVersion", project: "DEMO" });
    const [got] = reduce(open, { type: "nextVersion", version: "0.2.5" });
    expect(got.epic.suggested).toBe("0.2.5");
  });
});
