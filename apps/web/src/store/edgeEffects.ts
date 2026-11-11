// The network side of dependency editing: the daemon's edge routes, one request per change. It
// decides nothing; a refusal goes to the reducer with the daemon's own words.

import { del, post } from "../api/http";
import type { ChainDetail, EdgeKind } from "../api/types";
import { type EdgeEffect, idOf } from "./edges";
import type { Msg } from "./msg";

type Dispatch = (msg: Msg) => void;

const enc = encodeURIComponent;
const text = (e: unknown) => (e instanceof Error ? e.message : String(e));

/// Runs one effect of dependency editing.
export function runEdgeEffect(e: EdgeEffect, dispatch: Dispatch): void {
  const base = `/chains/${enc(e.chain)}/edges`;
  const add = (from: string, to: string, kind: EdgeKind) =>
    post<ChainDetail>(base, { from: idOf(from), to: idOf(to), kind });
  const remove = (from: string, to: string) =>
    del<ChainDetail>(`${base}?from=${enc(idOf(from))}&to=${enc(idOf(to))}`);
  const refused = (err: unknown) => dispatch({ type: "edgeRefused", message: text(err) });
  const changed = (chain: ChainDetail, from?: string, to?: string) =>
    dispatch({
      type: "edgesChanged",
      chain,
      selected: from && to ? { chain: e.chain, from, to } : undefined,
    });
  switch (e.type) {
    case "addEdge":
      add(e.from, e.to, "normal").then((chain) => changed(chain, e.from, e.to)).catch(refused);
      return;
    case "removeEdge":
      remove(e.from, e.to).then((chain) => changed(chain)).catch(refused);
      return;
    case "moveEdge": {
      const [from, to] = e.end === "from" ? [e.target, e.to] : [e.from, e.target];
      // One request: the daemon drops the old line and adds the new one in one transaction, so a
      // refusal leaves the chain as it was.
      post<ChainDetail>(`${base}/move`, {
        old: { from: idOf(e.from), to: idOf(e.to) },
        new: { from: idOf(from), to: idOf(to), kind: e.kind },
      })
        .then((chain) => changed(chain, from, to))
        .catch(refused);
      return;
    }
  }
}
