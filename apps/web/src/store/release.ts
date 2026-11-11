// Releasing a finished chain from the window: the Release dialog of the chain's end node. The
// daemon offers a version (suggested from the last tag) and the steps the project has, and runs
// them only after the person's click; this keeps its offer, and the run's steps as they go.

import type { ChainDetail, ReleaseOffer, ReleaseRun } from "../api/types";
import type { Effect } from "./msg";
import { withChain } from "./plan";
import type { State } from "./state";

export interface ReleaseUi {
  /// What the dialog offers; `undefined` until the daemon answered.
  offer: ReleaseOffer | undefined;
  /// The version in the field.
  version: string;
  /// The click was sent and not yet answered.
  busy: boolean;
  /// The daemon's refusal, as it said it.
  error: string | undefined;
  /// The last run of the release, as the daemon reports it.
  run: ReleaseRun | undefined;
}

export const initialRelease: ReleaseUi = { offer: undefined, version: "", busy: false, error: undefined, run: undefined };

/// What the person did.
export type ReleaseIntent =
  | { type: "openRelease"; chain: string }
  | { type: "setReleaseVersion"; version: string }
  /// `resume`: run an interrupted release again from its step; what passed is not redone.
  | { type: "startRelease"; chain: string; version: string; resume?: boolean };

/// What the daemon answered.
export type ReleaseResult =
  | { type: "releaseOffered"; offer: ReleaseOffer }
  | { type: "releaseStarted"; chain: ChainDetail; release: string }
  | { type: "releaseRun"; run: ReleaseRun }
  | { type: "releaseRefused"; message: string };

export type ReleaseMsg = ReleaseIntent | ReleaseResult;

export type ReleaseEffect =
  | { type: "loadReleaseOffer"; chain: string }
  | Extract<ReleaseIntent, { type: "startRelease" }>
  | { type: "pollRelease"; release: string };

/// The chain's end node offers Release when every card is merged and the release is not out yet.
export const canRelease = (chain: ChainDetail): boolean => chain.summary.releasable;

/// The reducer of releasing.
export function reduceRelease(s: State, msg: ReleaseMsg): [State, Effect[]] {
  const set = (release: Partial<ReleaseUi>, base: State = s): State => ({ ...base, release: { ...base.release, ...release } });
  switch (msg.type) {
    case "openRelease":
      return [
        { ...s, release: initialRelease, plan: { ...s.plan, dialog: { kind: "release", chain: msg.chain } } },
        [{ type: "loadReleaseOffer", chain: msg.chain }],
      ];
    case "releaseOffered":
      // The steps come from the journal: a run that is going, or one a restart cut off, shows.
      return [
        set({ offer: msg.offer, version: s.release.version || msg.offer.version, run: msg.offer.run ?? undefined }),
        msg.offer.run?.state === "running" ? [{ type: "pollRelease", release: msg.offer.run.release }] : [],
      ];
    case "setReleaseVersion":
      return [set({ version: msg.version }), []];
    case "startRelease":
      return [set({ busy: true, error: undefined, run: undefined }), [msg]];
    case "releaseStarted":
      return [set({ busy: false }, withChain(s, msg.chain)), [{ type: "pollRelease", release: msg.release }]];
    case "releaseRun":
      return [set({ run: msg.run }), []];
    case "releaseRefused":
      return [set({ busy: false, error: msg.message }), []];
  }
}
