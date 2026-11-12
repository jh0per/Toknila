import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ChainDetail, Project } from "../api/types";
import { epicTitle, titleByName } from "../store/title";
import { initialState } from "../store/state";
import { Roadmap } from "./Roadmap";
import { Sidebar } from "./Sidebar";

const project = { id: "pa", key: "AAA", name: "AAA name", archived: false } as unknown as Project;

/// A chain as the daemon sends it: `display_title` is the daemon's word for what a person reads.
const chain = (id: string, name: string, display: string, title?: string, position = 0): ChainDetail =>
  ({
    chain: { id, name, title, project_id: "pa", autostart: true, position },
    tickets: [], releases: [], edges: [], release_tickets: [], order: [],
    summary: { status: "idle", phase: "idle", total: 2, done: 1, finished_at: null, display_title: display },
  }) as unknown as ChainDetail;

const world = () => {
  vi.stubGlobal("window", {
    location: { search: "" },
    localStorage: { getItem: () => null, setItem: () => {} },
    matchMedia: () => ({ matches: false }),
  });
  return {
    ...initialState({ name: "project", project: "AAA" } as never),
    projects: [project],
    chains: {
      a: chain("a", "010-midi-controllers", "Midi controllers", undefined, 0),
      b: chain("b", "android-release", "Android release", "Android release", 1),
    },
    chainOrder: ["a", "b"],
  };
};

describe("an epic's title", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("is the daemon's display title; the name is the fallback of an older daemon", () => {
    expect(epicTitle(chain("a", "010-midi-controllers", "Midi controllers"))).toBe("Midi controllers");
    expect(epicTitle({ chain: { name: "x", title: "The x" } } as never)).toBe("The x");
    expect(epicTitle({ chain: { name: "x" } } as never)).toBe("x");
    expect(titleByName(world(), "010-midi-controllers")).toBe("Midi controllers");
    expect(titleByName(world(), "gone")).toBe("gone");
  });

  it("is what the sidebar row reads; the slug stays in the machine's attribute", () => {
    const h = renderToStaticMarkup(<Sidebar state={world()} dispatch={() => {}} />);
    expect(h).toContain('<span class="t chain-name">Midi controllers</span>');
    expect(h).toContain('<span class="t chain-name">Android release</span>');
    expect(h).toContain('data-chain-item="010-midi-controllers"');
    expect(h).not.toContain(">010-midi-controllers<");
  });

  it("is what the roadmap node reads; the slug stays in the machine's attribute", () => {
    const s = world();
    const h = renderToStaticMarkup(<Roadmap project={project} state={s} dispatch={() => {}} />);
    expect(h).toContain('data-testid="roadmap-name">Midi controllers</span>');
    expect(h).toContain('data-testid="roadmap-name">Android release</span>');
    expect(h).toContain('data-chain="010-midi-controllers"');
    expect(h).not.toContain(">010-midi-controllers<");
    expect(h).toContain('aria-label="Menu of Midi controllers"');
  });
});
