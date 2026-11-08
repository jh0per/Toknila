// Changing whether a chain starts its cards automatically from the window (the sidebar menu and the chain header). The daemon
// decides and answers with the chain; turning it on may start ready cards, which is the owner's
// act. A refusal is shown as the daemon said it.

import type { ChainDetail } from "../api/types";
import type { Effect } from "./msg";
import { withChain } from "./plan";
import type { State } from "./state";

/// What the person did.
export type AutostartIntent = { type: "setAutostart"; chain: string; autostart: boolean };

/// What the daemon answered.
export type AutostartResult =
  | { type: "autostartSet"; chain: ChainDetail }
  | { type: "autostartRefused"; message: string };

export type AutostartEffect = AutostartIntent;

/// The hint on the toggle: what turning it on does.
export const AUTOSTART_HINT =
  "On: ready cards start by themselves, and turning it on starts the ready ones now. The planner cannot change the cards' order or dependencies in a chain that starts cards automatically.";

/// The hint on the chain header's Run chain / Stop chain button: what each does.
export const RUN_CHAIN_HINT =
  "Run chain starts cards automatically and starts the ready cards up to the free agent slots. Stop chain stops starting cards automatically; cards already running keep running.";

/// The reducer of the autostart toggle.
export function reduceAutostart(s: State, msg: AutostartIntent | AutostartResult): [State, Effect[]] {
  switch (msg.type) {
    case "setAutostart":
      return [{ ...s, error: "" }, [msg]];
    case "autostartSet":
      // The agents' slots may have changed with the cards that started.
      return [{ ...withChain(s, msg.chain), error: "" }, [{ type: "loadAgents" }]];
    case "autostartRefused":
      return [{ ...s, error: msg.message }, []];
  }
}
