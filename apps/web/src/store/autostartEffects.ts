// The network side of the autostart toggle: one PATCH of the chain.

import { patch } from "../api/http";
import type { ChainDetail } from "../api/types";
import type { AutostartEffect } from "./autostart";
import type { Msg } from "./msg";

/// Runs the effect: asks the daemon, reports its answer or its refusal.
export function runAutostartEffect(e: AutostartEffect, dispatch: (msg: Msg) => void): void {
  patch<ChainDetail>(`/chains/${encodeURIComponent(e.chain)}`, { autostart: e.autostart })
    .then((chain) => dispatch({ type: "autostartSet", chain }))
    .catch((err) => dispatch({ type: "autostartRefused", message: err instanceof Error ? err.message : String(err) }));
}
