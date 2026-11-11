import { describe, expect, it } from "vitest";
import type { ChainDetail, InboxItem, Project } from "../api/types";
import { groupInbox, itemRoute, itemKey } from "./inbox";
import { initialState } from "./state";
import { reduce } from "./reduce";

const item = (kind: InboxItem["kind"], ticket: string, since: number, extra: Partial<InboxItem> = {}): InboxItem =>
  ({ kind, ticket, chain: "c1", title: "t", since, text: "", thread: null, subtasks: [], done_cards: [], ...extra }) as InboxItem;

describe("groupInbox", () => {
  it("groups by kind in a fixed order and keeps the newest-first order inside a group", () => {
    const items = [
      item("leftovers", "DEMO-1", 90),
      item("blocked", "DEMO-2", 80),
      item("question", "DEMO-3", 70),
      item("blocked", "DEMO-4", 60),
      item("proposal", "DEMO-5", 50),
    ];
    const groups = groupInbox(items).map((g) => [g.kind, g.items.map((i) => i.ticket)]);
    expect(groups).toEqual([
      ["proposal", ["DEMO-5"]],
      ["question", ["DEMO-3"]],
      ["blocked", ["DEMO-2", "DEMO-4"]],
      ["leftovers", ["DEMO-1"]],
    ]);
  });

  it("is empty for an empty inbox", () => {
    expect(groupInbox([])).toEqual([]);
  });
});

describe("a pin no card owns", () => {
  const pin = (id: string) => item("pin", undefined as never, 5, { chain: undefined, pin: id, project: "p1" });

  it("is its own group after the questions of the cards, keyed by the pin, with no route", () => {
    const groups = groupInbox([pin("a"), item("design_question", "DEMO-1", 6, { pin: "b" })]);
    expect(groups.map((g) => g.kind)).toEqual(["design_question", "pin"]);
    expect(itemKey(pin("a"))).not.toBe(itemKey(pin("b")));
    expect(itemRoute(initialState({ name: "chains" }), pin("a"))).toBeUndefined();
  });
});

describe("itemKey", () => {
  it("tells two threads of one card apart", () => {
    expect(itemKey(item("thread", "DEMO-1", 1, { thread: 1 }))).not.toBe(itemKey(item("thread", "DEMO-1", 1, { thread: 2 })));
  });
});

describe("itemRoute", () => {
  const projects = [{ id: "p1", key: "DEMO", name: "Demo" }] as Project[];
  const chain = { chain: { id: "c1", name: "demo", project_id: "p1" }, tickets: [] } as unknown as ChainDetail;

  it("opens the card of the item", () => {
    const [s] = reduce(initialState({ name: "chains" }), { type: "loaded", projects, chains: [chain] });
    expect(itemRoute(s, item("review", "DEMO-1", 1))).toEqual({
      name: "card", project: "DEMO", chain: "demo", key: "DEMO-1",
    });
  });

  it("has no route for a chain the page does not know", () => {
    expect(itemRoute(initialState({ name: "chains" }), item("review", "DEMO-1", 1))).toBeUndefined();
  });
});
