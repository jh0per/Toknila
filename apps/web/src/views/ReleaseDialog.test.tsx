import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ChainDetail, ReleaseOffer, ReleaseRun } from "../api/types";
import { initialRelease, plannedSteps, releasePush, type ReleaseUi } from "../store/release";
import { initialState } from "../store/state";
import { ReleaseDialog } from "./ReleaseDialog";

const chain = {
  chain: { id: "c1", name: "work", goal: "", autostart: true, project_id: "p1" },
  tickets: [], releases: [], edges: [], release_tickets: [], order: [],
  summary: { status: "idle", phase: "idle", finished_at: null, total: 1, done: 1, released: 0, releasable: true },
} as unknown as ChainDetail;

const offer = (extra: Partial<ReleaseOffer>): ReleaseOffer => ({
  releasable: true, version: "0.3.4", last_tag: "v0.3.3", steps: ["tag"], base: "main",
  remote: "git@github.com:acme/toknila.git", push: true, ...extra,
});

const render = (release: Partial<ReleaseUi>) => {
  const base = initialState({ name: "chains" });
  const state = { ...base, chains: { c1: chain }, release: { ...initialRelease, version: "0.3.4", ...release } };
  return renderToStaticMarkup(<ReleaseDialog chain="c1" state={state} dispatch={() => {}} />);
};

describe("the Release dialog's push checkbox", () => {
  it("is checked by default when the offer says so and names the remote's push", () => {
    const html = render({ offer: offer({ steps: ["tag", "push"] }) });
    expect(html).toMatch(/data-testid="release-push"[^>]*checked/);
    expect(html).toContain("Push main and the tag to origin");
    expect(html).toMatch(/data-step="push"/);
  });

  it("is unchecked by default for a remote whose base has no upstream, and the push step is not listed", () => {
    const html = render({ offer: offer({ push: false }) });
    expect(html).toContain('data-testid="release-push"');
    expect(html).not.toMatch(/data-testid="release-push"[^>]*checked/);
    expect(html).not.toMatch(/data-step="push"/);
  });

  it("follows the person's choice over the default", () => {
    const html = render({ offer: offer({ push: false }), push: true });
    expect(html).toMatch(/data-testid="release-push"[^>]*checked/);
    expect(html).toMatch(/data-step="push"/);
  });

  it("is absent without a remote, and one line says the tag stays local", () => {
    const html = render({ offer: offer({ remote: null, push: false }), push: true });
    expect(html).not.toContain('data-testid="release-push"');
    expect(html).toContain("no remote: the tag stays local");
    expect(html).not.toMatch(/data-step="push"/);
  });

  it("offers Push again, not Release again, when only the push failed, and links a pushed tag", () => {
    const failed: ReleaseRun = {
      release: "r1", version: "0.3.4", tag: "v0.3.4", state: "failed", error: "Push failed: rejected: non-fast-forward; pull first",
      steps: [
        { step: "tag", label: "Tag", state: "done", output: "Tagged v0.3.4" },
        { step: "push", label: "Push", state: "failed", output: "rejected: non-fast-forward; pull first" },
      ],
    };
    const html = render({ offer: offer({}), run: failed });
    expect(html).toContain("Push failed: rejected: non-fast-forward; pull first");
    expect(html).toMatch(/data-testid="release-confirm"[^>]*>Push again</);
    const done = render({
      offer: offer({}),
      run: {
        ...failed, state: "done", error: undefined,
        steps: [
          failed.steps[0],
          { step: "push", label: "Pushed main and v0.3.4 to origin", state: "done", output: "Pushed main and v0.3.4 to origin\nhttps://github.com/acme/toknila/releases/tag/v0.3.4" },
        ],
      },
    });
    expect(done).toContain('<a href="https://github.com/acme/toknila/releases/tag/v0.3.4"');
    expect(done).toContain("Pushed main and v0.3.4 to origin");
  });

  it("says what a step that cannot start waits for", () => {
    const run: ReleaseRun = {
      release: "r1", version: "0.3.8", tag: "v0.3.8", state: "running", turn: true,
      steps: [
        { step: "check", label: "Check", state: "running", output: "", wait: "waits for its turn (#2 in the check line)" },
        { step: "tag", label: "Tag", state: "pending", output: "" },
      ],
    };
    const html = render({ offer: offer({ steps: ["check", "tag"] }), run });
    expect(html).toContain("Check: waits for its turn (#2 in the check line)");
    const merging = render({
      offer: offer({ steps: ["check", "tag"] }),
      run: { ...run, turn: false, steps: [{ ...run.steps[0], state: "pending", wait: "waits for TOKNILA-152 to merge (#2 in line)" }, run.steps[1]] },
    });
    expect(merging).toContain("Check: waits for TOKNILA-152 to merge (#2 in line)");
    expect(merging).not.toContain("Tag:");
  });

  it("lists Build last with the command's first words, after the push when there is one", () => {
    const withBuild = offer({ steps: ["tag", "push", "build"], build: "./scripts/release-mac.sh --notarize" });
    const html = render({ offer: withBuild });
    expect(html).toContain("Build: ./scripts/release-mac.sh --notarize");
    expect(html.indexOf('data-step="push"')).toBeLessThan(html.indexOf('data-step="build"'));
    expect(plannedSteps({ ...initialRelease, offer: withBuild })).toEqual(["tag", "push", "build"]);
    expect(plannedSteps({ ...initialRelease, offer: withBuild, push: false })).toEqual(["tag", "build"]);
  });

  it("offers Build again, not Release again, when only the build failed", () => {
    const failed: ReleaseRun = {
      release: "r1", version: "0.3.4", tag: "v0.3.4", state: "failed", error: "Build failed: exit code 1",
      steps: [
        { step: "tag", label: "Tag", state: "done", output: "Tagged v0.3.4" },
        { step: "build", label: "Build: make dist", state: "failed", output: "exit code 1" },
      ],
    };
    const html = render({ offer: offer({ steps: ["tag", "build"], build: "make dist" }), run: failed });
    expect(html).toMatch(/data-testid="release-confirm"[^>]*>Build again</);
    expect(html).toContain("Build: make dist");
  });

  it("derives the choice from the offer in the store", () => {
    expect(releasePush({ ...initialRelease, offer: offer({ push: true }) })).toBe(true);
    expect(releasePush({ ...initialRelease, offer: offer({ remote: null, push: true }) })).toBe(false);
    expect(plannedSteps({ ...initialRelease, offer: offer({ steps: ["tag", "push"] }), push: false })).toEqual(["tag"]);
  });
});
