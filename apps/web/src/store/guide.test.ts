import { describe, expect, it } from "vitest";
import type { ChainDetail, InboxItem, Project } from "../api/types";
import { showMe } from "./guide";
import { GUIDE_SECTIONS, NO_PROJECT_REASON } from "./guideText";
import { reduce } from "./reduce";
import { initialState } from "./state";

const projects = [{ id: "p1", key: "DEMO", name: "Demo" }] as Project[];
const chain = { chain: { id: "c1", name: "demo", project_id: "p1" }, summary: { releasable: false }, tickets: [] } as unknown as ChainDetail;
const loaded = (inbox: InboxItem[] = []) => {
  const [s] = reduce(initialState({ name: "guide" }), { type: "loaded", projects, chains: [chain] });
  return { ...s, inbox };
};
const empty = () => reduce(initialState({ name: "guide" }), { type: "loaded", projects: [], chains: [] })[0];

describe("Show me", () => {
  it("is off, with the reason, for every section when there is no project", () => {
    for (const sec of GUIDE_SECTIONS) expect(showMe(empty(), sec.show)).toEqual({ off: NO_PROJECT_REASON });
  });

  it("opens the project's assistant screen for Plan with the assistant", () => {
    const plan = GUIDE_SECTIONS.find((s) => s.id === "plan");
    expect(plan && showMe(loaded(), plan.show)).toEqual({ route: { name: "planner", project: "DEMO" } });
  });

  it("leads to the project's own places", () => {
    const s = loaded();
    expect(showMe(s, "designs")).toEqual({ route: { name: "settings", project: "DEMO", section: "designs" } });
    expect(showMe(s, "inbox")).toEqual({ route: { name: "inbox", project: "DEMO" } });
    expect(showMe(s, "chain")).toEqual({ route: { name: "projectChain", project: "DEMO", chain: "demo" } });
    expect(showMe(s, "tree")).toEqual({ route: { name: "project", project: "DEMO" } });
    expect(showMe(s, "release")).toEqual({ route: { name: "projectChain", project: "DEMO", chain: "demo" } });
    expect(showMe(s, "addProject")).toEqual({ dialog: { kind: "addProject" } });
  });

  it("opens a card waiting for review, else the chain", () => {
    const review = { kind: "review", ticket: "DEMO-1", chain: "c1" } as InboxItem;
    expect(showMe(loaded([review]), "review")).toEqual({ route: { name: "card", project: "DEMO", chain: "demo", key: "DEMO-1" } });
    expect(showMe(loaded(), "review")).toEqual({ route: { name: "projectChain", project: "DEMO", chain: "demo" } });
  });
});

describe("the text of the guide", () => {
  it("explains Accept or Return in its own section, with the phone-check example", () => {
    const sec = GUIDE_SECTIONS.find((s) => s.id === "accept-or-return");
    expect(sec?.title).toBe("Review: Accept or Return");
    expect(sec?.text).toContain("Accept merges the story.");
    expect(sec?.text).toContain("Return sends your words to the agent, which fixes and hands in again.");
    expect(sec?.text).toContain("on a phone");
  });

  it("has the nine sections in order, two to four sentences each, and never names the reference", () => {
    expect(GUIDE_SECTIONS.map((s) => s.id)).toEqual(["add-project", "designs", "plan", "start", "inbox", "review", "accept-or-return", "release", "where"]);
    for (const sec of GUIDE_SECTIONS) {
      const sentences = sec.text.split(/(?<=[.!?])\s+(?=[A-Z"])/).length;
      expect(sentences).toBeGreaterThanOrEqual(2);
      expect(sentences).toBeLessThanOrEqual(4);
    }
  });
});
