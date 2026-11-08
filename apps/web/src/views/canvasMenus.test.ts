import { describe, expect, it, vi } from "vitest";
import type { TicketAction, TicketDetail } from "../api/types";
import { cancelEntries, canvasEntries, cardEntries } from "./canvasMenus";

const detail = (allowed: TicketAction[], can_move: boolean): TicketDetail =>
  ({ allowed_actions: allowed, can_move }) as unknown as TicketDetail;
const handlers = () => ({ open: vi.fn(), act: vi.fn(), addAfter: vi.fn(), addBefore: vi.fn(), askCancel: vi.fn() });
const labels = (d: TicketDetail | undefined) => cardEntries(d, handlers()).map((e) => e.label);

describe("the card menu", () => {
  it("offers only Open and Add card after until the daemon's word on the card is known", () => {
    expect(labels(undefined)).toEqual(["Open", "Add card after"]);
  });

  it("follows the allowed actions and can_move of the daemon", () => {
    expect(labels(detail(["start", "pause", "cancel"], true))).toEqual(
      ["Open", "Start", "Pause", "Add card after", "Add card before", "Cancel"],
    );
    expect(labels(detail(["resume", "cancel"], false))).toEqual(["Open", "Resume", "Add card after", "Cancel"]);
    expect(labels(detail([], false))).toEqual(["Open", "Add card after"]);
  });

  it("runs the action it names, and Cancel only asks", () => {
    const h = handlers();
    const e = cardEntries(detail(["start", "cancel"], true), h);
    e.find((x) => x.testId === "menu-card-start")?.run();
    expect(h.act).toHaveBeenCalledWith("start");
    e.find((x) => x.testId === "menu-card-cancel")?.run();
    expect(h.askCancel).toHaveBeenCalled();
    expect(h.act).toHaveBeenCalledTimes(1);
  });

  it("asks before cancelling, and keeping the card runs nothing", () => {
    const run = vi.fn();
    const e = cancelEntries("DEMO-1", run);
    expect(e.map((x) => x.label)).toEqual(["Cancel DEMO-1", "Keep it"]);
    e[1].run();
    expect(run).not.toHaveBeenCalled();
    e[0].run();
    expect(run).toHaveBeenCalledTimes(1);
  });
});

describe("the canvas menu", () => {
  it("has the four entries", () => {
    const h = { newCard: vi.fn(), newChain: vi.fn(), importPlan: vi.fn(), askPlanner: vi.fn() };
    const e = canvasEntries(h);
    expect(e.map((x) => x.label)).toEqual(["New card", "New chain", "Import a plan", "Ask the planner"]);
    e[3].run();
    expect(h.askPlanner).toHaveBeenCalled();
  });
});
