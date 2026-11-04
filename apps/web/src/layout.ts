// Pure canvas layout: chain read model -> node positions. Column = longest dependency path from
// a root; nodes of one column sit in separate lanes; the same input always gives the same output.

import type { ChainDetail, NodeRef } from "./api/types";

export interface LaidNode {
  kind: NodeRef["kind"];
  id: string;
  col: number;
  lane: number;
}

export interface LaidEdge {
  from: string;
  to: string;
}

export interface Layout {
  nodes: LaidNode[];
  /// Edges between `nodeKey`s.
  edges: LaidEdge[];
  columns: number;
  lanes: number;
}

/// The identity of a node across tickets and releases.
export const nodeKey = (n: { kind: string; id: string }): string => `${n.kind}:${n.id}`;

/// Positions of every ticket and release of a chain. The daemon's `order` is the priority and
/// breaks ties; nodes it does not list follow in the order of tickets, then releases.
export function layoutChain(c: ChainDetail): Layout {
  const listed: NodeRef[] = [
    ...c.order,
    ...c.tickets.map((t): NodeRef => ({ kind: "ticket", id: t.id })),
    ...c.releases.map((r): NodeRef => ({ kind: "release", id: r.id })),
  ];
  const rank = new Map<string, number>();
  const refs: NodeRef[] = [];
  for (const n of listed) {
    if (rank.has(nodeKey(n))) continue;
    rank.set(nodeKey(n), refs.length);
    refs.push(n);
  }
  const edges = c.edges
    .map((e) => ({ from: nodeKey(e.from), to: nodeKey(e.to) }))
    .filter((e) => rank.has(e.from) && rank.has(e.to) && e.from !== e.to);
  const preds = new Map<string, string[]>();
  const succs = new Map<string, string[]>();
  for (const e of edges) {
    preds.set(e.to, [...(preds.get(e.to) ?? []), e.from]);
    succs.set(e.from, [...(succs.get(e.from) ?? []), e.to]);
  }

  // Longest path from a root, by Kahn's algorithm; a cycle (which the daemon refuses) stays at 0.
  const keys = refs.map(nodeKey);
  const col = new Map<string, number>(keys.map((k) => [k, 0]));
  const left = new Map<string, number>(keys.map((k) => [k, preds.get(k)?.length ?? 0]));
  const queue = keys.filter((k) => left.get(k) === 0);
  for (let i = 0; i < queue.length; i++) {
    const k = queue[i];
    for (const s of succs.get(k) ?? []) {
      col.set(s, Math.max(col.get(s) ?? 0, (col.get(k) ?? 0) + 1));
      left.set(s, (left.get(s) ?? 1) - 1);
      if (left.get(s) === 0) queue.push(s);
    }
  }

  // Lanes: a node continues the lane of its first predecessor when free, else the lowest free one.
  const sorted = [...refs].sort(
    (a, b) =>
      (col.get(nodeKey(a)) ?? 0) - (col.get(nodeKey(b)) ?? 0) ||
      (rank.get(nodeKey(a)) ?? 0) - (rank.get(nodeKey(b)) ?? 0),
  );
  const lane = new Map<string, number>();
  const used = new Map<number, Set<number>>();
  const nodes = sorted.map((n): LaidNode => {
    const k = nodeKey(n);
    const c = col.get(k) ?? 0;
    const taken = used.get(c) ?? new Set<number>();
    const first = (preds.get(k) ?? []).map((p) => lane.get(p)).find((l) => l !== undefined);
    let l = first !== undefined && !taken.has(first) ? first : 0;
    while (taken.has(l)) l++;
    taken.add(l);
    used.set(c, taken);
    lane.set(k, l);
    return { kind: n.kind, id: n.id, col: c, lane: l };
  });
  return {
    nodes,
    edges,
    columns: nodes.reduce((m, n) => Math.max(m, n.col + 1), 0),
    lanes: nodes.reduce((m, n) => Math.max(m, n.lane + 1), 0),
  };
}
