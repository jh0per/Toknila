import { useEffect, useRef, useState } from "react";
import type { ChainDetail } from "../api/types";
import { type LaidEdge, layoutChain, nodeKey } from "../layout";
import { outOfOrderText } from "../format";
import { statusGroup, statusText } from "../store/groups";
import { type EdgeRef, idOf } from "../store/edges";
import type { Intent } from "../store/msg";
import { type State, currentProject, hrefOf, queuePosition } from "../store/state";
import { ContextMenu } from "./ContextMenu";
import { EdgeLayer } from "./EdgeLayer";
import { PrChip } from "./PrChip";
import { UsageChip } from "./UsageChip";
import { H, W, canvasSize, inPoint, outPoint, x, y } from "./canvasGeometry";
import { type Drag, useEdgeDrag } from "./useEdgeDrag";

/// The chain canvas: cards left to right in dependency order, releases as their own nodes. The
/// dependencies can be edited with the mouse: drag from a card's handle onto another card to add
/// one, click a line to select it, drag a selected line's end onto another card to move it,
/// Delete or the right-click menu to remove it. The daemon decides; its refusal shows on top.
export function Canvas({ chain, state, dispatch }: {
  chain: ChainDetail;
  state: State;
  dispatch: (i: Intent) => void;
}) {
  const layout = layoutChain(chain);
  const chainId = chain.chain.id;
  const projectKey = currentProject(state)?.key ?? chain.chain.project_id;
  const at = new Map(layout.nodes.map((n) => [nodeKey(n), n]));
  const tickets = new Map(chain.tickets.map((t) => [t.id, t]));
  const releases = new Map(chain.releases.map((r) => [r.id, r]));
  const label = (k: string) => {
    const id = idOf(k);
    return tickets.get(id)?.key ?? releases.get(id)?.version ?? id;
  };
  const selected: LaidEdge | undefined = layout.edges.find((e) => {
    const s = state.edges.selected;
    return s?.chain === chainId && s.from === e.from && s.to === e.to;
  });
  const select = (e: LaidEdge | undefined) =>
    dispatch({ type: "selectEdge", edge: e && { chain: chainId, from: e.from, to: e.to } satisfies EdgeRef });
  const [menu, setMenu] = useState<{ x: number; y: number; edge: LaidEdge }>();
  const inner = useRef<HTMLDivElement>(null);

  const drop = (d: Drag, target: string) => {
    if (d.mode === "new") {
      if (target !== d.node) dispatch({ type: "addEdge", chain: chainId, from: d.node, to: target });
      return;
    }
    if (target === (d.end === "from" ? d.edge.from : d.edge.to)) return;
    const kind = chain.edges.find((e) => nodeKey(e.from) === d.edge.from && nodeKey(e.to) === d.edge.to)?.kind ?? "normal";
    dispatch({ type: "moveEdge", chain: chainId, from: d.edge.from, to: d.edge.to, kind, end: d.end, target });
  };
  const { drag, start } = useEdgeDrag(inner, drop);

  // Delete or Backspace removes the selected line, Escape lets go of it; typing in a field is left alone.
  useEffect(() => {
    if (!selected) return;
    const key = (ev: KeyboardEvent) => {
      const t = ev.target as HTMLElement | null;
      if (t?.closest("input, textarea, select, [contenteditable=true], [role=dialog]")) return;
      if (ev.key === "Delete" || ev.key === "Backspace") {
        ev.preventDefault();
        dispatch({ type: "removeEdge", chain: chainId, from: selected.from, to: selected.to });
      } else if (ev.key === "Escape") {
        select(undefined);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  const { width, height } = canvasSize(layout.columns, layout.lanes);
  const sel = state.route.name === "card" ? state.route.key : "";
  return (
    <div className={`canvas${drag ? " dragging" : ""}`} data-testid="canvas" data-chain={chain.chain.name}>
      {state.edges.error && (
        <div className="edge-error" role="alert" data-testid="edge-error">
          <span>{state.edges.error}</span>
          <button type="button" className="link-btn" onClick={() => dispatch({ type: "dismissEdgeError" })}>Dismiss</button>
        </div>
      )}
      <div className="canvas-inner" ref={inner} style={{ width, height }} onClick={() => select(undefined)}>
        {layout.nodes.map((n) => {
          const style = { left: x(n.col), top: y(n.lane), width: W, height: H };
          const over = drag?.over === nodeKey(n) ? " over" : "";
          if (n.kind === "release") {
            const r = releases.get(n.id);
            if (!r) return null;
            return (
              <div key={nodeKey(n)} className={`node release${over}`} data-node={nodeKey(n)} data-version={r.version} style={style}>
                <b className="key">◆ {r.version}</b>
                <span className="muted">{r.channel} · {r.start_mode}</span>
                <small className="muted" data-testid="release-state">{r.state}</small>
              </div>
            );
          }
          const t = tickets.get(n.id);
          if (!t) return null;
          const route = { name: "card" as const, project: projectKey, chain: chain.chain.name, key: t.key };
          return (
            <a
              key={nodeKey(n)}
              className={`node ticket g-${statusGroup(t.status)}${sel === t.key ? " sel" : ""}${over}`}
              data-key={t.key}
              data-node={nodeKey(n)}
              href={hrefOf(route)}
              draggable={false}
              style={style}
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                dispatch({ type: "selectCard", project: route.project, chain: route.chain, key: t.key });
              }}
            >
              <span className="row">
                <span className="title">{t.title}</span>
                {t.profile && <span className="runner">{t.profile}</span>}
              </span>
              <span className="row">
                <span className="st" data-testid="status">{statusText(t.status)}</span>
                <UsageChip figures={chain.usage?.[t.key]} />
                <span className="key">{t.key}</span>
              </span>
              {queuePosition(state, t.id) !== undefined && (
                <span className="mark queued" data-testid="queued" title="Waiting for a free agent slot">
                  queued #{queuePosition(state, t.id)}
                </span>
              )}
              {t.pr && <PrChip pr={t.pr} />}
              {t.done_out_of_order && (
                <span className="mark" title={outOfOrderText(t.done_out_of_order)}>out of order</span>
              )}
            </a>
          );
        })}
        {layout.nodes.map((n) => (
          <span
            key={`h-${nodeKey(n)}`} className="handle" data-node={nodeKey(n)} data-testid={`handle-${label(nodeKey(n))}`}
            title="Drag onto another card to make it wait for this one"
            style={{ left: outPoint(n).x - 6, top: outPoint(n).y - 6 }}
            onPointerDown={(e) => start({ mode: "new", node: nodeKey(n), anchor: outPoint(n) }, e)}
          />
        ))}
        <EdgeLayer
          width={width} height={height} edges={layout.edges} at={at} label={label} selected={selected} drag={drag}
          onSelect={select}
          onMenu={(edge, p) => {
            select(edge);
            setMenu({ ...p, edge });
          }}
          onGrab={(e, end, ev) =>
            start(
              {
                mode: "move",
                edge: { from: e.from, to: e.to },
                end,
                // The end that stays: the other one.
                anchor: end === "from" ? inPoint(at.get(e.to) ?? { col: 0, lane: 0 }) : outPoint(at.get(e.from) ?? { col: 0, lane: 0 }),
              },
              ev,
            )}
        />
      </div>
      {menu && (
        <ContextMenu
          x={menu.x} y={menu.y} onClose={() => setMenu(undefined)}
          entries={[{
            label: "Remove dependency",
            testId: "menu-remove-edge",
            danger: true,
            run: () => dispatch({ type: "removeEdge", chain: chainId, from: menu.edge.from, to: menu.edge.to }),
          }]}
        />
      )}
    </div>
  );
}
