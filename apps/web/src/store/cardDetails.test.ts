import { describe, expect, it } from "vitest";
import type { ChainDetail, EventRecord, TicketDetail, TicketStatus } from "../api/types";
import { allows, detailsWanted, subtaskProgress } from "./cardDetails";
import { reduce } from "./reduce";
import { initialState } from "./state";

const ticket = (n: number, status: TicketStatus) => ({ id: `t${n}`, key: `D-${n}`, status, chain_id: "c1", title: `Card ${n}` });
const chain = (...tickets: ReturnType<typeof ticket>[]) =>
  ({
    chain: { id: "c1", name: "work", project_id: "p1" },
    tickets, releases: [], edges: [], release_tickets: [], order: [],
    summary: { status: "running", total: tickets.length, done: 0, released: 0 },
  }) as unknown as ChainDetail;
const detail = (n: number, done: boolean[], allowed: string[] = []) =>
  ({ ticket: ticket(n, "running"), subtasks: done.map((d) => ({ done: d })), allowed_actions: allowed }) as unknown as TicketDetail;

describe("subtaskProgress", () => {
  it("counts the done ones, and says nothing for a card without subtasks", () => {
    expect(subtaskProgress(detail(1, [true, true, false, false, false]))).toEqual({ done: 2, total: 5 });
    expect(subtaskProgress(detail(1, []))).toBeUndefined();
    expect(subtaskProgress(undefined)).toBeUndefined();
  });
});

describe("allows", () => {
  it("goes by the daemon's allowed actions only", () => {
    expect(allows(detail(1, [], ["approve", "reject"]), "approve")).toBe(true);
    expect(allows(detail(1, [], ["start"]), "approve")).toBe(false);
    expect(allows(undefined, "approve")).toBe(false);
  });
});

describe("detailsWanted", () => {
  const s0 = initialState({ name: "chains" });
  const c = chain(ticket(1, "backlog"), ticket(2, "running"), ticket(3, "proposed"), ticket(4, "waiting"), ticket(5, "merged"));

  it("asks for the cards that are proposed or at work, not for the rest", () => {
    expect(detailsWanted(s0, [c])).toEqual([
      { type: "loadDetail", key: "D-2" }, { type: "loadDetail", key: "D-3" }, { type: "loadDetail", key: "D-4" },
    ]);
  });

  it("does not ask again for a card it has read, unless everything is reloaded", () => {
    const s = { ...s0, details: { "D-2": detail(2, []) } };
    expect(detailsWanted(s, [c]).map((e) => (e as { key: string }).key)).toEqual(["D-3", "D-4"]);
    expect(detailsWanted(s, [c], true)).toHaveLength(3);
  });
});

describe("the store", () => {
  it("fetches the details of the cards at work when a chain arrives", () => {
    const [, fx] = reduce(initialState({ name: "chains" }), { type: "chainLoaded", chain: chain(ticket(1, "running"), ticket(2, "ready")) });
    expect(fx).toEqual([{ type: "loadDetail", key: "D-1" }]);
  });

  it("refreshes a known card's detail when a subtask of it changes", () => {
    const s = { ...initialState({ name: "chains" }), lastSeq: 4, details: { "D-1": detail(1, [false]) } };
    const withChain = reduce(s, { type: "chainLoaded", chain: chain(ticket(1, "running")) })[0];
    const e = { seq: 5, entity: "subtask", kind: "updated", entity_id: "s1", payload: { subtask: { ticket_id: "t1" } } } as unknown as EventRecord;
    const [, fx] = reduce(withChain, { type: "event", event: e });
    expect(fx).toContainEqual({ type: "loadDetail", key: "D-1" });
  });
});
