import { describe, expect, it, vi } from "vitest";
import type { TicketAction, TicketDetail } from "../api/types";
import { canvasEntries, cardEntries, edgeMenu } from "./canvasMenus";

const detail = (allowed: TicketAction[], can_move: boolean, can_edit = false): TicketDetail =>
  ({ allowed_actions: allowed, can_move, can_edit }) as unknown as TicketDetail;
const handlers = () => ({
  open: vi.fn(), openTerminal: vi.fn(), edit: vi.fn(), act: vi.fn(), ask: vi.fn(), addAfter: vi.fn(),
  addBefore: vi.fn(), addDependency: vi.fn(), askCancel: vi.fn(),
});
/// Labels with `|` where a separator is drawn.
const shape = (d: TicketDetail | undefined) =>
  cardEntries(d, handlers()).map((e) => `${e.sep ? "|" : ""}${e.label}`);

describe("the card menu", () => {
  it("offers only what needs no state until the daemon's word on the card is known", () => {
    expect(shape(undefined)).toEqual(["Open", "Open terminal", "|Add story after"]);
  });

  it("is grouped: open | the daemon's actions | around the card | cancel", () => {
    expect(shape(detail(["start", "pause", "cancel"], true, true))).toEqual([
      "Open", "Edit", "Open terminal", "|Start", "Pause", "|Add story before", "Add story after", "Add dependency…", "|Cancel",
    ]);
  });

  it("follows the daemon: no Edit unless can_edit, no Add before or dependency unless can_move", () => {
    expect(shape(detail(["resume", "cancel"], false))).toEqual(["Open", "Open terminal", "|Resume", "|Add story after", "|Cancel"]);
    expect(shape(detail([], false))).toEqual(["Open", "Open terminal", "|Add story after"]);
  });

  it("offers a proposed card Approve and Reject, and a card in review Accept and Return", () => {
    expect(shape(detail(["approve", "reject"], false))).toEqual(["Open", "Open terminal", "|Approve", "Reject…", "|Add story after"]);
    expect(shape(detail(["accept", "return", "reopen"], false)).slice(2, 6)).toEqual(["|Accept", "Return…", "Reopen", "|Add story after"]);
  });

  it("has at most one red entry and it is last", () => {
    const e = cardEntries(detail(["approve", "reject", "cancel"], true, true), handlers());
    const red = e.map((x, i) => (x.danger ? i : -1)).filter((i) => i >= 0);
    expect(red).toEqual([e.length - 1]);
  });

  it("offers Ask for a design among the actions, before Cancel, and runs ask_design", () => {
    const h = handlers();
    const d = detail(["start", "pause", "ask_design", "cancel"], true);
    expect(shape(d)).toContain("|Start");
    expect(shape(d).filter((l) => l.includes("design"))).toEqual(["Ask for a design"]);
    cardEntries(d, h).find((x) => x.testId === "menu-card-ask_design")?.run();
    expect(h.act).toHaveBeenCalledWith("ask_design");
    expect(shape(detail(["accept", "return"], false)).join()).not.toContain("design");
  });

  it("shows the shortcuts the diagram has: Enter opens, S starts", () => {
    const e = cardEntries(detail(["start"], false), handlers());
    expect(e.find((x) => x.testId === "menu-card-open")?.keys).toBe("Enter");
    expect(e.find((x) => x.testId === "menu-card-start")?.keys).toBe("S");
  });

  it("runs the action it names; Reject, Return and Cancel only ask", () => {
    const h = handlers();
    const e = cardEntries(detail(["start", "reject", "return", "cancel"], true, true), h);
    const run = (id: string) => e.find((x) => x.testId === id)?.run();
    run("menu-card-start");
    expect(h.act).toHaveBeenCalledWith("start");
    run("menu-card-reject");
    expect(h.ask).toHaveBeenLastCalledWith("reject");
    run("menu-card-return");
    expect(h.ask).toHaveBeenLastCalledWith("return");
    run("menu-card-cancel");
    expect(h.askCancel).toHaveBeenCalled();
    run("menu-card-terminal");
    run("menu-card-edit");
    run("menu-card-add-dependency");
    expect(h.openTerminal).toHaveBeenCalled();
    expect(h.edit).toHaveBeenCalled();
    expect(h.addDependency).toHaveBeenCalled();
    expect(h.act).toHaveBeenCalledTimes(1);
  });

  it("does not offer Block: it needs a reason the menu has no field for", () => {
    expect(shape(detail(["block", "cancel"], false))).not.toContain("|Block");
    expect(shape(detail(["block"], false)).join()).not.toContain("Block");
  });
});

describe("the canvas menu", () => {
  it("has its entries in three groups, New card first with its key", () => {
    const h = { newCard: vi.fn(), newChain: vi.fn(), importPlan: vi.fn(), askPlanner: vi.fn() };
    const e = canvasEntries(h);
    expect(e.map((x) => `${x.sep ? "|" : ""}${x.label}`)).toEqual(["New story", "|New epic", "Import a plan", "|Plan with the assistant"]);
    expect(e[0].keys).toBe("N");
    e[3].run();
    expect(h.askPlanner).toHaveBeenCalled();
  });
});

describe("the line menu", () => {
  it("is titled with the two cards and has one red entry, Remove dependency, with its key", () => {
    const remove = vi.fn();
    const m = edgeMenu("TOK-1", "TOK-2", remove);
    expect(m.title).toBe("TOK-1 → TOK-2");
    expect(m.entries.map((x) => [x.label, x.keys, x.danger])).toEqual([["Remove dependency", "Delete", true]]);
    m.entries[0].run();
    expect(remove).toHaveBeenCalled();
  });
});
