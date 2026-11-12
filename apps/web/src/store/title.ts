// What a person reads for an epic. The daemon decides it (`summary.display_title`: the title, or
// the name made readable); the page shows it and never repeats the rule. The slug (`chain.name`)
// is the machine's: folders, branches, the address of a page.

import type { ChainDetail } from "../api/types";
import type { State } from "./state";

type Chain = ChainDetail["chain"];

/// The words for an epic: the daemon's `display_title`, else the stored title, else the name (a
/// chain an older daemon sent).
export const epicTitle = (c: { chain: Chain; summary?: { display_title?: string } }): string =>
  c.summary?.display_title || c.chain.title || c.chain.name;

/// The words for the epic of this name, as the store knows it; the name itself when it does not.
export function titleByName(s: State, name: string): string {
  for (const id of s.chainOrder) {
    const c = s.chains[id];
    if (c?.chain.name === name) return epicTitle(c);
  }
  return name;
}
