import { describe, expect, it } from "vitest";
import type { TicketDetail } from "../api/types";
import { RUN_CHAIN_HINT } from "./autostart";
import { CANCEL_REASON } from "./quick";
import { reduce } from "./reduce";
import { initialState } from "./state";

const start = () => initialState({ name: "chains" });
const open = (extra: object = {}) =>
  reduce(start(), { type: "openQuickCard", chain: "c1", x: 40, y: 50, ...extra })[0];
const detail = (key: string): TicketDetail =>
  ({ ticket: { key, chain_id: "c1" }, allowed_actions: ["start"], can_move: true }) as unknown as TicketDetail;

describe("the quick new-card field in the store", () => {
  it("opens where the person pointed and forgets the last refusal", () => {
    const refused = reduce(start(), { type: "planRejected", form: "card", field: undefined, message: "no" })[0];
    const [s, fx] = reduce(refused, { type: "openQuickCard", chain: "c1", x: 40, y: 50, after: "DEMO-1" });
    expect(s.quick.card).toEqual({ chain: "c1", x: 40, y: 50, after: "DEMO-1", before: undefined });
    expect(s.plan.error).toBeUndefined();
    expect(fx).toEqual([]);
  });

  it("closes", () => {
    const [s] = reduce(open(), { type: "closeQuickCard" });
    expect(s.quick.card).toBeUndefined();
  });

  it("asks the daemon for a card after the one the line came from, with the title trimmed", () => {
    const [s, fx] = reduce(open({ after: "DEMO-1" }), { type: "submitQuickCard", title: "  Next  " });
    expect(fx).toEqual([{ type: "createCard", chain: "c1", input: { title: "Next", after: ["DEMO-1"] } }]);
    expect(s.plan.busy).toBe(true);
    expect(s.quick.card).toBeDefined();
  });

  it("asks for a card that comes before another as a dependency of it", () => {
    const [, fx] = reduce(open({ before: "DEMO-2" }), { type: "submitQuickCard", title: "First" });
    expect(fx).toEqual([{ type: "createCard", chain: "c1", input: { title: "First", before: ["DEMO-2"] } }]);
  });

  it("appends a card without a relation at the end of the chain", () => {
    const [, fx] = reduce(open(), { type: "submitQuickCard", title: "Last" });
    expect(fx).toEqual([{ type: "createCard", chain: "c1", input: { title: "Last", at_end: true } }]);
  });

  it("does nothing when no field is open", () => {
    const [s, fx] = reduce(start(), { type: "submitQuickCard", title: "x" });
    expect(fx).toEqual([]);
    expect(s.plan.busy).toBe(false);
  });

  it("keeps the field open with the daemon's words when it refuses, and closes it when the card is made", () => {
    const [busy] = reduce(open({ after: "DEMO-1" }), { type: "submitQuickCard", title: "x" });
    const [refused] = reduce(busy, { type: "planRejected", form: "card", field: undefined, message: "DEMO-1 is gone" });
    expect(refused.quick.card).toBeDefined();
    expect(refused.plan.error?.message).toBe("DEMO-1 is gone");
    const [made] = reduce(busy, { type: "cardCreated", detail: detail("DEMO-9") });
    expect(made.quick.card).toBeUndefined();
  });
});

describe("reading a card's allowed actions", () => {
  it("reads the card again when asked, and trusts it once the answer is in", () => {
    const [asked, fx] = reduce(start(), { type: "peekCard", key: "DEMO-1" });
    expect(fx).toEqual([{ type: "loadDetail", key: "DEMO-1" }]);
    expect(asked.quick.peek).toEqual({ key: "DEMO-1", fresh: false });
    const [other] = reduce(asked, { type: "detailLoaded", detail: detail("DEMO-2") });
    expect(other.quick.peek?.fresh).toBe(false);
    const [fresh] = reduce(asked, { type: "detailLoaded", detail: detail("DEMO-1") });
    expect(fresh.quick.peek).toEqual({ key: "DEMO-1", fresh: true });
    expect(fresh.details["DEMO-1"]).toBeDefined();
  });
});

describe("Run chain", () => {
  it("uses the autostart switch and says both what Run and Stop do", () => {
    const [, fx] = reduce(start(), { type: "setAutostart", chain: "c1", autostart: true });
    expect(fx).toEqual([{ type: "setAutostart", chain: "c1", autostart: true }]);
    expect(RUN_CHAIN_HINT).toMatch(/starts the ready cards up to the free agent slots/);
    expect(RUN_CHAIN_HINT).toMatch(/Stop chain stops starting cards automatically; cards already running keep running/);
  });
});

describe("cancelling a card", () => {
  it("sends the reason the person confirmed, and starts from the prefilled one", () => {
    expect(CANCEL_REASON).toBe("Cancelled in the window");
    const [, fx] = reduce(start(), { type: "ticketAction", key: "DEMO-1", action: "cancel", text: "not needed any more" });
    expect(fx).toEqual([{ type: "ticketAction", key: "DEMO-1", action: "cancel", text: "not needed any more" }]);
  });
});
