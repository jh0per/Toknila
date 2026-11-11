// The network side of releasing: the offer, the click, and the poll of the run's steps. It decides
// nothing: the daemon's answers go to the reducer.

import { get, post } from "../api/http";
import type { ChainDetail, ReleaseOffer, ReleaseRun } from "../api/types";
import type { Msg } from "./msg";
import type { ReleaseEffect } from "./release";

type Dispatch = (msg: Msg) => void;

/// How often the open dialog asks for the steps of a running release.
const POLL_MS = 600;
/// A run that is still going after this long is no longer followed.
const GIVE_UP_MS = 60 * 60 * 1000;

const refused = (dispatch: Dispatch) => (e: unknown) =>
  dispatch({ type: "releaseRefused", message: e instanceof Error ? e.message : String(e) });

async function follow(release: string, dispatch: Dispatch): Promise<void> {
  const until = Date.now() + GIVE_UP_MS;
  while (Date.now() < until) {
    const run = await get<ReleaseRun>(`/releases/${encodeURIComponent(release)}/run`);
    dispatch({ type: "releaseRun", run });
    if (run.state !== "running") return;
    await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }
}

/// Runs one effect of releasing.
export function runReleaseEffect(e: ReleaseEffect, dispatch: Dispatch): void {
  switch (e.type) {
    case "loadReleaseOffer":
      get<ReleaseOffer>(`/chains/${encodeURIComponent(e.chain)}/release`)
        .then((offer) => dispatch({ type: "releaseOffered", offer }))
        .catch(refused(dispatch));
      return;
    case "startRelease":
      post<ChainDetail>(`/chains/${encodeURIComponent(e.chain)}/release`, { version: e.version })
        .then((chain) => {
          // The release that was just cut is the one whose steps are followed.
          const cut = [...chain.releases].reverse().find((r) => r.state === "cut");
          if (cut) dispatch({ type: "releaseStarted", chain, release: cut.id });
          else dispatch({ type: "releaseRefused", message: "The release was not cut." });
        })
        .catch(refused(dispatch));
      return;
    case "pollRelease":
      follow(e.release, dispatch).catch(refused(dispatch));
      return;
  }
}
