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

describe("SidebarChains as drop targets", () => {
  beforeEach(() => vi.stubGlobal("window", { location: { search: "" } }));
  afterEach(() => vi.unstubAllGlobals());
  const rows = (drop?: string) =>
    renderToStaticMarkup(
      <SidebarChains
        chains={[chain("now", "active"), chain("next", "waiting")]} project="DEMO" selected={undefined}
        dispatch={() => {}} onMenu={() => () => {}} drop={drop}
      />,
    );
  it("tells every row's epic and project, so a drag can find where it is", () => {
    const html = rows();
    expect(html).toContain('data-chain-id="now"');
    expect(html).toContain('data-chain-id="next"');
    expect(html.match(/data-chain-project="DEMO"/g)).toHaveLength(2);
    expect(html).not.toContain("data-drop");
  });
  it("marks only the row a dragged card is over", () => {
    const html = rows("next");
    expect(html.match(/data-drop="true"/g)).toHaveLength(1);
    expect(html).toMatch(/<li[^>]*data-chain-id="next"[^>]*data-drop="true"/);
    expect(html).not.toMatch(/<li[^>]*data-chain-id="now"[^>]*data-drop/);
  });
});

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
  it("opens the group, dimmed, for a selected finished chain", () => {
    const html = render("old");
    expect(html).toContain('data-chain-item="old"');
    expect(html).toContain("dim");
    expect(html).not.toContain("12 Nov");
  });
  it("renders a long finished chain's name, no date, no phase word, in one line", () => {
    const name = "a-very-long-chain-name-that-cannot-fit-in-the-sidebar-width";
    const html = renderToStaticMarkup(
      <SidebarChains chains={[chain(name, "finished")]} project="DEMO" selected={name} dispatch={() => {}} onMenu={() => () => {}} />,
    );
    expect(html).toContain(`<span class="t chain-name">${name}</span>`);
    expect(html).toContain(`title="${name}"`);
    expect(html).not.toContain("12 Nov");
    expect(html).not.toContain("phase-word");
    expect(html).not.toContain("finished</span>");
    expect(html).toContain("4/4");
    expect(html).not.toContain("<br");
  });
  it("shows the phase word only for an active chain", () => {
    expect(render("now")).toContain('<span class="phase-word">active</span>');
  });
});

describe("PhaseChip", () => {
  it("prints the phase word", () => {
    const html = renderToStaticMarkup(<PhaseChip phase="finished" />);
    expect(html).toContain('data-phase="finished"');
    expect(html).toContain("finished");
  });
});
