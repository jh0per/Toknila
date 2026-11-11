import { describe, expect, it } from "vitest";
import type { ChainDetail, NodeRef } from "./api/types";
import { type Layout, layoutChain } from "./layout";

const t = (id: string): NodeRef => ({ kind: "ticket", id });
const r = (id: string): NodeRef => ({ kind: "release", id });

function chain(order: NodeRef[], edges: [NodeRef, NodeRef][]): ChainDetail {
  return {
    chain: { id: "c", name: "demo", goal: "", autostart: true, project_id: "p" },
    tickets: order.filter((n) => n.kind === "ticket").map((n) => ({ id: n.id })),
    releases: order.filter((n) => n.kind === "release").map((n) => ({ id: n.id })),
    edges: edges.map(([from, to]) => ({ chain_id: "c", from, to })),
    release_tickets: [],
    order,
    summary: { status: "running", phase: "idle", finished_at: null, total: 0, done: 0, released: 0 },
  } as unknown as ChainDetail;
}

const pos = (l: Layout) => Object.fromEntries(l.nodes.map((n) => [n.id, [n.col, n.lane]]));

// examples/demo.toml: A -> B and D in parallel -> release R1 -> C.
const demoEdges: [NodeRef, NodeRef][] = [
  [t("A"), t("B")], [t("A"), t("D")], [t("B"), r("R1")], [t("D"), r("R1")], [r("R1"), t("C")],
];
const demo = () => chain([t("A"), t("B"), t("D"), r("R1"), t("C")], demoEdges);

describe("layoutChain", () => {
  it("puts the demo diamond in four columns with the parallel branch in its own lane", () => {
    const l = layoutChain(demo());
    expect(pos(l)).toEqual({ A: [0, 0], B: [1, 0], D: [1, 1], R1: [2, 0], C: [3, 0] });
    expect(l.columns).toBe(4);
    expect(l.lanes).toBe(2);
    expect(l.nodes.find((n) => n.id === "R1")?.kind).toBe("release");
    expect(l.edges).toHaveLength(5);
  });

  it("uses the longest path for the column", () => {
    const l = layoutChain(chain([t("A"), t("B"), t("C")], [[t("A"), t("B")], [t("B"), t("C")], [t("A"), t("C")]]));
    expect(pos(l)).toEqual({ A: [0, 0], B: [1, 0], C: [2, 0] });
  });

  it("is deterministic and follows the given order for ties", () => {
    expect(layoutChain(demo())).toEqual(layoutChain(demo()));
    const swapped = chain([t("A"), t("D"), t("B"), r("R1"), t("C")], demoEdges);
    expect(pos(layoutChain(swapped))).toMatchObject({ D: [1, 0], B: [1, 1] });
  });

  it("gives independent roots separate lanes and survives nodes missing from the order", () => {
    expect(pos(layoutChain(chain([t("X"), t("Y")], [])))).toEqual({ X: [0, 0], Y: [0, 1] });
    const c = chain([t("X")], []);
    c.tickets = [{ id: "X" }, { id: "Z" }] as ChainDetail["tickets"];
    expect(pos(layoutChain(c))).toEqual({ X: [0, 0], Z: [0, 1] });
  });

  it("does not hang on a cycle", () => {
    const l = layoutChain(chain([t("A"), t("B")], [[t("A"), t("B")], [t("B"), t("A")]]));
    expect(l.nodes).toHaveLength(2);
  });
});
