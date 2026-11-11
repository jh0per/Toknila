// Pure scene of a chain's canvas: the cards and releases as laid out, plus the Start node on the
// left and the end node on the right, with the implicit lines that join every card to them. The
// implicit lines are derived on every draw and never stored.

import type { ChainDetail, Pos, Size } from "./api/types";
import { type LaidEdge, layoutChain, nodeKey } from "./layout";
import { type Box, GAP_Y, H, PAD, W, columnXs, laneYs } from "./views/canvasGeometry";

/// The size a card is drawn at, and whether it is the card's own (a person gave it).
export type CardSize = Size & { own: boolean };

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
  /// The card has a size of its own.
  sized: boolean;
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
  // The end release has nothing before it until its cards are wired to it: Start reaches it only
  // when it is the chain's only node.
  for (const k of nodes) if (!hasBefore.has(k) && (k !== end || inner.length === 0)) lines.push({ from: START, to: k, implicit: true });
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
/// column 1, the end node after the last of them. Each column is as wide as its widest node and
/// each lane as tall as its tallest (a card that was dragged away still counts in its own
/// column and lane, so dragging one never moves the others), and Start and the end node sit at
/// the middle of that height. `sizeOf` gives a card's size; a node without one is `W` x `H`.
export function sceneOf(
  c: ChainDetail,
  posOf: (ticketId: string) => Pos | undefined,
  sizeOf: (ticketId: string) => CardSize | undefined = () => undefined,
): Scene {
  const lay = layoutChain(c);
  const keys = lay.nodes.map(nodeKey);
  const endKey = endRelease(c, lay.edges, keys) ?? FINISH;
  const lastCol = lay.nodes.reduce((m, n) => (nodeKey(n) === endKey ? m : Math.max(m, n.col + 1)), 0);
  const card = (n: { kind: string; id: string }): CardSize =>
    (n.kind === "ticket" ? sizeOf(n.id) : undefined) ?? { w: W, h: H, own: false };
  const widths = Array.from({ length: lastCol + 2 }, () => W);
  const tall = Array.from({ length: Math.max(1, lay.lanes) }, () => 0);
  for (const n of lay.nodes) {
    const col = nodeKey(n) === endKey ? lastCol + 1 : n.col + 1;
    widths[col] = Math.max(widths[col], card(n).w);
    tall[n.lane] = Math.max(tall[n.lane], card(n).h);
  }
  // A lane with no node (the chain is empty) is as tall as a node of the bookends.
  const heights = tall.map((h) => h || H);
  const xs = columnXs(widths);
  const ys = laneYs(heights);
  const total = heights.reduce((a, h) => a + h, 0) + GAP_Y * (heights.length - 1);
  const mid = PAD + Math.round((total - H) / 2);
  const nodes: SceneNode[] = [
    { key: START, kind: "start", id: c.chain.name, col: 0, lane: 0, box: { x: xs[0], y: mid, w: W, h: H }, placed: false, sized: false },
  ];
  for (const n of lay.nodes) {
    const key = nodeKey(n);
    if (key === endKey) continue;
    const at = n.kind === "ticket" ? posOf(n.id) : undefined;
    const col = n.col + 1;
    const { w, h, own } = card(n);
    nodes.push({
      key, kind: n.kind, id: n.id, col, lane: n.lane, placed: Boolean(at), sized: own,
      box: { x: at ? at.x : xs[col], y: at ? at.y : ys[n.lane], w, h },
    });
  }
  const end = lay.nodes.find((n) => nodeKey(n) === endKey);
  nodes.push({
    key: endKey, kind: end ? "release" : "finish", id: end?.id ?? c.chain.name,
    col: lastCol + 1, lane: 0, box: { x: xs[lastCol + 1], y: mid, w: W, h: H }, placed: false, sized: false,
  });
  return { nodes, edges: lay.edges, lines: bookendLines(keys, lay.edges, endKey) };
}
