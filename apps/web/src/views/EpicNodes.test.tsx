import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ChainDetail, ChainSummary } from "../api/types";
import { versionText } from "../store/phase";
import { FinishNode, StartNode } from "./CanvasNodes";
import { SidebarChains } from "./SidebarChains";

const summary = (over: Partial<ChainSummary>): ChainSummary =>
  ({ status: "idle", phase: "idle", finished_at: null, total: 4, done: 1, released: 0, releasable: false, ...over }) as ChainSummary;

const start = (goal: string) =>
  renderToStaticMarkup(<StartNode name="Checkout rework" goal={goal} style={{}} selected={false} onOpen={() => {}} />);

const finish = (s: ChainSummary, onRelease?: () => void) =>
  renderToStaticMarkup(<FinishNode style={{}} summary={s} onRelease={onRelease} onOpen={() => {}} />);

describe("the Start node is the epic", () => {
  it("shows the chain's name as the title and its goal as the description", () => {
    const html = start("Let customers pay with **saved cards**.\n\n- wallets\n- the old flow behind a flag");
    expect(html).toContain('data-testid="epic-title"');
    expect(html).toContain("Checkout rework");
    expect(html).toContain("<b>saved cards</b>");
    expect(html).toContain("<li><span>wallets</span></li>");
    expect(html).not.toContain("<script");
  });
  it("puts the rest of a long goal behind more, and a short one needs none", () => {
    expect(start("Four\n\nlines\n\nof\n\ngoal")).toContain('data-testid="epic-more"');
    expect(start("A short goal.")).not.toContain('data-testid="epic-more"');
  });
  it("says so when there is no goal yet", () => {
    expect(start("  ")).toContain("No goal yet");
  });
  it("never injects markup from the goal", () => {
    const html = start("<img src=x onerror=alert(1)>");
    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;img");
  });
});

describe("the Finish node is the release", () => {
  const f = (state: "planned" | "ready" | "released", extra = {}) =>
    summary({ finish: { version: "0.3.0", state, ...extra }, version: "0.3.0", releasable: state === "ready" });

  it("reads Release v0.3.0 planned while cards are open, with the cards left", () => {
    const html = finish(f("planned"));
    expect(html).toContain("Release v0.3.0");
    expect(html).toContain('data-state="planned"');
    expect(html).toContain(">planned<");
    expect(html).toContain("3 left");
    expect(html).not.toContain("release-button");
  });
  it("reads ready to release with the Release button once every card is merged", () => {
    const html = finish(f("ready"), () => {});
    expect(html).toContain("Release v0.3.0");
    expect(html).toContain("ready to release");
    expect(html).toContain('data-testid="release-button"');
  });
  it("reads released with the date and the tag", () => {
    const html = finish(f("released", { tag: "v0.3.0", released_at: Date.UTC(2026, 10, 12, 12) }));
    expect(html).toContain(">released<");
    expect(html).toContain("12 Nov · v0.3.0");
  });
  it("keeps the phase's words for a chain without a version", () => {
    const html = finish(summary({ phase: "active" }));
    expect(html).not.toContain("Release v");
    expect(html).toContain("3 left");
  });
});

describe("the version beside the chain's name", () => {
  beforeEach(() => vi.stubGlobal("window", { location: { search: "" } }));
  afterEach(() => vi.unstubAllGlobals());
  const chain = (version?: string) =>
    ({ chain: { id: "a", name: "auth" }, summary: summary({ version, total: 4, done: 1 }) }) as unknown as ChainDetail;
  it("is v plus the version, or nothing", () => {
    expect(versionText({ version: "0.3.0" })).toBe("v0.3.0");
    expect(versionText({})).toBe("");
  });
  it("shows in the sidebar row after the name", () => {
    const html = renderToStaticMarkup(
      <SidebarChains chains={[chain("0.3.0")]} project="DEMO" selected={undefined} dispatch={() => {}} onMenu={() => () => {}} />,
    );
    expect(html.indexOf("auth")).toBeLessThan(html.indexOf("v0.3.0"));
    const bare = renderToStaticMarkup(
      <SidebarChains chains={[chain()]} project="DEMO" selected={undefined} dispatch={() => {}} onMenu={() => () => {}} />,
    );
    expect(bare).not.toContain("chain-version-row");
  });
});
