import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ChainDetail } from "../api/types";
import { PhaseChip } from "./PhaseChip";
import { SidebarChains } from "./SidebarChains";

const chain = (name: string, phase: ChainDetail["summary"]["phase"]) =>
  ({ chain: { id: name, name }, summary: { phase, total: 4, done: phase === "finished" ? 4 : 1, finished_at: phase === "finished" ? Date.UTC(2026, 10, 12, 12) : null } }) as unknown as ChainDetail;

const render = (selected?: string) =>
  renderToStaticMarkup(
    <SidebarChains
      chains={[chain("old", "finished"), chain("now", "active"), chain("next", "waiting")]}
      project="DEMO" selected={selected} dispatch={() => {}} onMenu={() => () => {}}
    />,
  );

describe("SidebarChains", () => {
  beforeEach(() => vi.stubGlobal("window", { location: { search: "" } }));
  afterEach(() => vi.unstubAllGlobals());
  it("shows the word and the count, active before waiting, and folds finished chains", () => {
    const html = render("now");
    expect(html).toContain('data-phase="active"');
    expect(html).toContain("1/4");
    expect(html.indexOf("now")).toBeLessThan(html.indexOf("next"));
    expect(html).toContain("Finished (1)");
    expect(html).not.toContain('data-chain-item="old"');
  });
  it("opens the group, dimmed with the date, for a selected finished chain", () => {
    const html = render("old");
    expect(html).toContain('data-chain-item="old"');
    expect(html).toContain("dim");
    expect(html).toContain("12 Nov");
  });
});

describe("PhaseChip", () => {
  it("prints the phase word", () => {
    const html = renderToStaticMarkup(<PhaseChip phase="finished" />);
    expect(html).toContain('data-phase="finished"');
    expect(html).toContain("finished");
  });
});
