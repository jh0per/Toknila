import { describe, expect, it } from "vitest";
import { reduce } from "./reduce";
import { initialState } from "./state";
import { type UpdateView, banner, lastCheckText } from "./updates";

const view = (over: Partial<UpdateView> = {}): UpdateView => ({
  current_version: "0.2.0",
  last_check: 1_700_000_000,
  busy: false,
  banner: { title: "Toknila 0.2.1 is ready", notes: "Quieter cards" },
  error: null,
  ...over,
});

const start = () => initialState({ name: "chains" });

describe("updates", () => {
  it("starts watching the shell with the page", () => {
    const [, fx] = reduce(start(), { type: "started" });
    expect(fx).toContainEqual({ type: "watchUpdates" });
  });

  it("shows the banner only when the shell reports a ready update", () => {
    const [s0] = reduce(start(), { type: "updateViewLoaded", view: view({ banner: null }) });
    expect(banner(s0.updates)).toBeUndefined();
    const [s1] = reduce(s0, { type: "updateViewLoaded", view: view() });
    expect(banner(s1.updates)).toEqual({ title: "Toknila 0.2.1 is ready", notes: "Quieter cards" });
    // In a browser the shell never answers: nothing to show.
    expect(banner(start().updates)).toBeUndefined();
  });

  it("restart asks the shell once, without confirmation", () => {
    const [s] = reduce(start(), { type: "updateViewLoaded", view: view() });
    const [, fx] = reduce(s, { type: "restartToUpdate" });
    expect(fx).toEqual([{ type: "restartToUpdate", confirmed: false }]);
  });

  it("a warning is shown, and only Restart anyway confirms", () => {
    const [s] = reduce(start(), { type: "updateViewLoaded", view: view() });
    const [warned] = reduce(s, { type: "updateWarned", message: "2 agents are running; restarting stops them" });
    expect(warned.updates.warning).toBe("2 agents are running; restarting stops them");
    const [anyway, fx] = reduce(warned, { type: "restartAnyway" });
    expect(fx).toEqual([{ type: "restartToUpdate", confirmed: true }]);
    expect(anyway.updates.warning).toBeUndefined();
  });

  it("Later hides this version's banner and drops the warning; a newer version shows again", () => {
    const [s] = reduce(start(), { type: "updateViewLoaded", view: view() });
    const [warned] = reduce(s, { type: "updateWarned", message: "x" });
    const [later, fx] = reduce(warned, { type: "updateLater" });
    expect(fx).toEqual([]);
    expect(later.updates.warning).toBeUndefined();
    expect(banner(later.updates)).toBeUndefined();
    const [newer] = reduce(later, {
      type: "updateViewLoaded",
      view: view({ banner: { title: "Toknila 0.2.2 is ready", notes: "" } }),
    });
    expect(banner(newer.updates)?.title).toBe("Toknila 0.2.2 is ready");
  });

  it("Check for updates asks the shell and keeps its failure", () => {
    const [s, fx] = reduce(start(), { type: "checkForUpdates" });
    expect(fx).toEqual([{ type: "checkForUpdates" }]);
    const [failed] = reduce(s, { type: "updateFailed", message: "offline" });
    expect(failed.updates.error).toBe("offline");
  });

  it("words the last check", () => {
    expect(lastCheckText(null)).toBe("Not checked yet");
    expect(lastCheckText(1_700_000_000)).toMatch(/^Last checked .*2023/);
  });
});
