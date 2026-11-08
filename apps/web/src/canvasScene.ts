// Pure scene of a chain's canvas: the cards and releases as laid out, plus the Start node on the
// left and the end node on the right, with the implicit lines that join every card to them. The
// implicit lines are derived on every draw and never stored.

import type { ChainDetail, Pos } from "./api/types";
import { type LaidEdge, layoutChain, nodeKey } from "./layout";
import { type Box, x, y } from "./views/canvasGeometry";

/// The node keys of the two bookends; a chain has one of each.
export const START = "start:chain";
export const FINISH = "finish:chain";

/// A line of the diagram. An implicit one only says "this card starts here" or "ends here".
export interface Line {
  from: string;
  to: string;
  implicit: boolean;
}

export interface SceneNode {
  key: string;
  kind: "ticket" | "release" | "start" | "finish";
  /// The id of the card or release; the chain's name for the bookends.
  id: string;
  /// Where it sits in the automatic layout.
  col: number;
  lane: number;
  /// Where it is drawn: the saved place of a card, else the automatic one.
  box: Box;
  /// The card has a place of its own.
  placed: boolean;
}

export interface Scene {
  nodes: SceneNode[];
  /// The dependencies.
  edges: LaidEdge[];
  /// Dependencies first, then the implicit lines from Start and to the end node.
  lines: Line[];
}

/// The implicit lines of a chain, after its dependencies: Start to every node with nothing
/// before it, and every node with nothing after it to `end` (a node key: the chain's last
/// release, which is then one of `nodes`, or the Finish node, which is not). A chain with no
/// nodes still gets Start to `end`, so the diagram always reads from in to out.
export function bookendLines(nodes: string[], edges: LaidEdge[], end: string): Line[] {
  const hasBefore = new Set(edges.map((e) => e.to));
  const hasAfter = new Set(edges.map((e) => e.from));
  const lines: Line[] = edges.map((e) => ({ ...e, implicit: false }));
  const inner = nodes.filter((k) => k !== end);
  for (const k of nodes) if (!hasBefore.has(k)) lines.push({ from: START, to: k, implicit: true });
  for (const k of inner) if (!hasAfter.has(k)) lines.push({ from: k, to: end, implicit: true });
  if (inner.length === 0 && !nodes.includes(end)) lines.push({ from: START, to: end, implicit: true });
  return lines;
}

/// The chain's end node among its nodes: its last release that nothing waits for, if it has one.
function endRelease(c: ChainDetail, edges: LaidEdge[], keys: string[]): string | undefined {
  const hasAfter = new Set(edges.map((e) => e.from));
  const sinks = keys.filter((k) => k.startsWith("release:") && !hasAfter.has(k));
  const order = c.order.map(nodeKey);
  return sinks.sort((a, b) => order.indexOf(a) - order.indexOf(b)).pop();
}

/// The whole canvas of a chain. `posOf` gives a card's own place (saved, or being dragged), if
/// any; every other node is placed by the automatic layout: Start in column 0, cards from
/// column 1, the end node after the last of them, Start and the end node in the middle lane.
export function sceneOf(c: ChainDetail, posOf: (ticketId: string) => Pos | undefined): Scene {
  const lay = layoutChain(c);
  const keys = lay.nodes.map(nodeKey);
  const endKey = endRelease(c, lay.edges, keys) ?? FINISH;
  const middle = Math.max(0, lay.lanes - 1) / 2;
  const lastCol = lay.nodes.reduce((m, n) => (nodeKey(n) === endKey ? m : Math.max(m, n.col + 1)), 0);
  const nodes: SceneNode[] = [
    { key: START, kind: "start", id: c.chain.name, col: 0, lane: middle, box: { x: x(0), y: y(middle) }, placed: false },
  ];
  for (const n of lay.nodes) {
    const key = nodeKey(n);
    if (key === endKey) continue;
    const own = n.kind === "ticket" ? posOf(n.id) : undefined;
    const col = n.col + 1;
    nodes.push({
      key, kind: n.kind, id: n.id, col, lane: n.lane, placed: Boolean(own),
      box: own ? { x: own.x, y: own.y } : { x: x(col), y: y(n.lane) },
    });
  }
  const end = lay.nodes.find((n) => nodeKey(n) === endKey);
  nodes.push({
    key: endKey, kind: end ? "release" : "finish", id: end?.id ?? c.chain.name,
    col: lastCol + 1, lane: middle, box: { x: x(lastCol + 1), y: y(middle) }, placed: false,
  });
  return { nodes, edges: lay.edges, lines: bookendLines(keys, lay.edges, endKey) };
}
