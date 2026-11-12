import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ChainDetail, Project, TodayUsage } from "../api/types";
import { reduce } from "../store/reduce";
import { resetClock, runningSpend } from "../store/spend";
import { initialState } from "../store/state";
import { Sidebar } from "./Sidebar";
import { SpendLine } from "./SpendLine";
import { UsageChip } from "./UsageChip";

const figures = (cost: number) => ({ cost_usd: cost, messages: 3, processed: 1_200_000 }) as never;
const today = (quota: TodayUsage["quota"]): TodayUsage => ({
  project: figures(23.77),
  machine: figures(57.77),
  quota,
  cards: { "AAA-1": figures(3.8), "AAA-2": figures(1) },
});
const FIVE = 4_102_444_800;
const quota = { windows: [
  { name: "seven_day", used_percentage: 8.2, resets_at: FIVE + 1000 },
  { name: "five_hour", used_percentage: 48.4, resets_at: FIVE },
] };

const project = { id: "pa", key: "AAA", name: "AAA", archived: false } as unknown as Project;
const chain = {
  chain: { id: "ca", name: "ca", project_id: "pa", autostart: true },
  tickets: [{ id: "ta", key: "AAA-1", title: "Card", status: "running" }],
  releases: [], edges: [], release_tickets: [], order: [],
  summary: { status: "running", total: 1, done: 0, released: 0 },
} as unknown as ChainDetail;
const world = (t?: TodayUsage) => {
  const s = {
    ...initialState({ name: "chain", project: "AAA", chain: "ca" } as never),
    projects: [project], chains: { ca: chain }, chainOrder: ["ca"],
    agents: [{ project: "pa", limit: 2, used: 1, queued: [], runs: [{ card: { id: "ta", key: "AAA-1", title: "Card" }, role: "worker", since: 1 }] }] as never,
  };
  return t ? reduce(s, { type: "todayLoaded", project: "AAA", view: t })[0] : s;
};
const text = (html: string) => html.replace(/<[^>]*>/g, "");

describe("the spend line of the top bar", () => {
  beforeEach(() => vi.stubGlobal("window", { location: { search: "" } }));
  afterEach(() => vi.unstubAllGlobals());

  it("says the project's estimated spend today and links to the Usage page, with the machine's in its title", () => {
    const html = renderToStaticMarkup(<SpendLine state={world(today(null))} dispatch={() => {}} />);
    expect(text(html)).toBe("est. $23.77 today");
    expect(html).toContain('href="/usage"');
    expect(html).toContain("all projects: $57.77 today");
    expect(html).toMatch(/title="[^"]*estimate/i);
    expect(html).not.toContain("spend-window");
  });

  it("adds the 5-hour window with its reset and the 7-day one when the daemon sent a quota", () => {
    const html = renderToStaticMarkup(<SpendLine state={world(today(quota))} dispatch={() => {}} />);
    expect(text(html)).toBe(`est. $23.77 today5h 48% used · resets ${resetClock(FIVE)}7d 8%`);
    expect(html.match(/data-testid="spend-window"/g)).toHaveLength(2);
  });

  it("shows nothing before an answer, or for another project's answer", () => {
    expect(renderToStaticMarkup(<SpendLine state={world()} dispatch={() => {}} />)).toBe("");
    const other = reduce(world(), { type: "todayLoaded", project: "BBB", view: today(null) })[0];
    expect(renderToStaticMarkup(<SpendLine state={other} dispatch={() => {}} />)).toBe("");
  });

  it("asks the daemon for the project's figures", () => {
    const [, fx] = reduce(world(), { type: "wantToday", project: "AAA" });
    expect(fx).toEqual([{ type: "loadToday", project: "AAA" }]);
  });
});

describe("what the running agents spent today", () => {
  beforeEach(() => vi.stubGlobal("window", { location: { search: "" } }));
  afterEach(() => vi.unstubAllGlobals());

  it("sums the running cards once each, whatever their role", () => {
    const run = (key: string, role: "worker" | "reviewer") => ({ card: { id: key, key, title: key }, role, since: 1 });
    const runs = [run("AAA-1", "worker"), run("AAA-1", "reviewer"), { card: null, role: "planner" as const, since: 1 }];
    expect(runningSpend(today(null), runs)).toBeCloseTo(3.8);
    expect(runningSpend(today(null), [])).toBeUndefined();
    expect(runningSpend(undefined, runs)).toBeUndefined();
  });

  it("shows next to the slot count in the Agents node, in the same style", () => {
    const html = renderToStaticMarkup(<Sidebar state={world(today(null))} dispatch={() => {}} />);
    expect(html).toMatch(/project-slots[^>]*>1 \/ 2<\/span><span class="side-spend" data-testid="agents-spend"[^>]*>est\. \$3\.80/);
    expect(renderToStaticMarkup(<Sidebar state={world()} dispatch={() => {}} />)).not.toContain("agents-spend");
  });
});

describe("the chip of a card", () => {
  it("shows its cost and the time of its current run while it runs, the sum of its runs otherwise", () => {
    const now = Date.now();
    const running = renderToStaticMarkup(<UsageChip figures={figures(3.8)} runTime={{ done_ms: 60_000, since: now - 137_000 }} />);
    expect(text(running)).toMatch(/^est\. \$3\.80 · 2m 1[78]s$/);
    const idle = renderToStaticMarkup(<UsageChip figures={figures(3.8)} runTime={{ done_ms: 95_000, since: null }} />);
    expect(text(idle)).toBe("est. $3.80 · 1m 35s");
    expect(renderToStaticMarkup(<UsageChip figures={undefined} runTime={undefined} />)).toBe("");
  });
});
