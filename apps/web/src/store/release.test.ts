import { describe, expect, it } from "vitest";
import type { ChainDetail, ReleaseOffer, ReleaseRun } from "../api/types";
import { canRelease } from "./release";
import { reduce } from "./reduce";
import { initialState } from "./state";

const chain = (releasable: boolean): ChainDetail =>
  ({
    chain: { id: "c1", name: "work", goal: "", autostart: true, project_id: "p1" },
    tickets: [], releases: [{ id: "r1", chain_id: "c1", version: "1.4.3", state: "cut" }], edges: [], release_tickets: [], order: [],
    summary: { status: "idle", phase: "idle", finished_at: null, total: 1, done: 1, released: 0, releasable },
  }) as unknown as ChainDetail;

const offer: ReleaseOffer = { releasable: true, version: "1.4.3", last_tag: "v1.4.2", steps: ["check", "tag"], base: "develop" };

describe("releasing a finished chain", () => {
  it("offers Release only to a chain whose cards are all merged", () => {
    expect(canRelease(chain(true))).toBe(true);
    expect(canRelease(chain(false))).toBe(false);
  });

  it("opens the dialog, asks for the offer and fills the version from it", () => {
    const [open, fx] = reduce(initialState({ name: "chains" }), { type: "openRelease", chain: "c1" });
    expect(open.plan.dialog).toEqual({ kind: "release", chain: "c1" });
    expect(fx).toEqual([{ type: "loadReleaseOffer", chain: "c1" }]);
    const [offered] = reduce(open, { type: "releaseOffered", offer });
    expect(offered.release.version).toBe("1.4.3");
  });

  it("sends the click with the version as typed and does nothing before it", () => {
    const [open] = reduce(initialState({ name: "chains" }), { type: "openRelease", chain: "c1" });
    const [typed] = reduce(open, { type: "setReleaseVersion", version: "2.0.0" });
    expect(typed.release.busy).toBe(false);
    const [s, fx] = reduce(typed, { type: "startRelease", chain: "c1", version: "2.0.0" });
    expect(s.release.busy).toBe(true);
    expect(fx).toEqual([{ type: "startRelease", chain: "c1", version: "2.0.0" }]);
  });

  it("follows the run of the release that was cut and keeps its steps", () => {
    const [started, fx] = reduce(initialState({ name: "chains" }), { type: "releaseStarted", chain: chain(true), release: "r1" });
    expect(fx).toEqual([{ type: "pollRelease", release: "r1" }]);
    expect(started.chains.c1).toBeDefined();
    const run: ReleaseRun = {
      release: "r1", version: "1.4.3", tag: "v1.4.3", state: "failed", error: "Check failed: exit code 3",
      steps: [{ step: "check", label: "Check", state: "failed", output: "boom" }, { step: "tag", label: "Tag", state: "pending", output: "" }],
    };
    const [s] = reduce(started, { type: "releaseRun", run });
    expect(s.release.run?.error).toBe("Check failed: exit code 3");
  });

  it("shows a refusal as the daemon said it and lets the person click again", () => {
    const [busy] = reduce(initialState({ name: "chains" }), { type: "startRelease", chain: "c1", version: "1.0.0" });
    const [s] = reduce(busy, { type: "releaseRefused", message: "the chain still has open cards" });
    expect(s.release).toMatchObject({ busy: false, error: "the chain still has open cards" });
  });
});

describe("a release that a restart cut off", () => {
  const interrupted: ReleaseRun = {
    release: "r1", version: "1.4.3", tag: "v1.4.3", state: "interrupted", resume_from: "bump",
    steps: [
      { step: "check", label: "Check", state: "done", output: "ok" },
      { step: "bump", label: "Bump the version", state: "interrupted", output: "" },
      { step: "tag", label: "Tag", state: "pending", output: "" },
    ],
  };

  it("shows the journaled steps from the offer and does not poll a run that stands still", () => {
    const [open] = reduce(initialState({ name: "chains" }), { type: "openRelease", chain: "c1" });
    const [s, fx] = reduce(open, { type: "releaseOffered", offer: { ...offer, run: interrupted } });
    expect(s.release.run?.resume_from).toBe("bump");
    expect(s.release.version).toBe("1.4.3");
    expect(fx).toEqual([]);
  });

  it("follows a run that is still going when the dialog opens", () => {
    const [open] = reduce(initialState({ name: "chains" }), { type: "openRelease", chain: "c1" });
    const [, fx] = reduce(open, { type: "releaseOffered", offer: { ...offer, run: { ...interrupted, state: "running", resume_from: undefined } } });
    expect(fx).toEqual([{ type: "pollRelease", release: "r1" }]);
  });

  it("sends the click to run it again with resume", () => {
    const [s, fx] = reduce(initialState({ name: "chains" }), { type: "startRelease", chain: "c1", version: "1.4.3", resume: true });
    expect(s.release.busy).toBe(true);
    expect(fx).toEqual([{ type: "startRelease", chain: "c1", version: "1.4.3", resume: true }]);
  });
});
