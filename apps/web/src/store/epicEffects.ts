// The network side of the epic and release panels. It decides nothing: answers go to the reducer.

import { get, patch } from "../api/http";
import type { ChainDetail, VersionSuggestion } from "../api/types";
import type { Msg } from "./msg";
import type { EpicEffect } from "./epic";

type Dispatch = (msg: Msg) => void;

/// Runs one effect of the epic and release panels.
export function runEpicEffect(e: EpicEffect, dispatch: Dispatch): void {
  switch (e.type) {
    case "saveEpic":
      patch<ChainDetail>(`/chains/${encodeURIComponent(e.chain)}`, e.patch)
        .then((chain) => dispatch({ type: "epicSaved", project: e.project, from: e.from, chain }))
        .catch((err: unknown) => dispatch({ type: "epicRefused", message: err instanceof Error ? err.message : String(err) }));
      return;
    case "loadNextVersion":
      // Only a suggestion: when it cannot be had, the field starts empty.
      get<VersionSuggestion>(`/projects/${encodeURIComponent(e.project)}/next-version`)
        .then((s) => dispatch({ type: "nextVersion", version: s.version }))
        .catch(() => undefined);
      return;
  }
}
