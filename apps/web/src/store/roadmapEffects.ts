// The network side of the roadmap. It decides nothing: answers go to the reducer.

import { post } from "../api/http";
import type { ChainDetail } from "../api/types";
import type { Msg } from "./msg";
import type { RoadmapEffect } from "./roadmap";

/// Runs one effect of the roadmap.
export function runRoadmapEffect(e: RoadmapEffect, dispatch: (msg: Msg) => void): void {
  post<ChainDetail>(`/chains/${encodeURIComponent(e.chain)}/move`, e.to)
    .then((chain) => dispatch({ type: "chainMoved", chain }))
    .catch((err: unknown) => dispatch({ type: "chainMoveRefused", message: err instanceof Error ? err.message : String(err) }));
}
