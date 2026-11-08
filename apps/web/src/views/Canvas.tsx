import { type MouseEvent as ReactMouseEvent, useEffect, useRef, useState } from "react";
import type { ChainDetail, TicketAction } from "../api/types";
import { type LaidEdge, layoutChain, nodeKey } from "../layout";
import { outOfOrderText } from "../format";
import { statusGroup, statusText } from "../store/groups";
import { type EdgeRef, idOf } from "../store/edges";
import type { Intent } from "../store/msg";
import { type State, currentProject, hrefOf, queuePosition } from "../store/state";
import { ContextMenu, type MenuEntry } from "./ContextMenu";
import { EdgeLayer } from "./EdgeLayer";
import { PrChip } from "./PrChip";
import { QuickCard } from "./QuickCard";
import { UsageChip } from "./UsageChip";
import { H, type Pt, W, canvasSize, inPoint, outPoint, x, y } from "./canvasGeometry";
import { CancelConfirm } from "./CancelConfirm";
import { canvasEntries, cardEntries } from "./canvasMenus";
import { useCanvasKeys } from "./useCanvasKeys";
import { type Drag, useEdgeDrag } from "./useEdgeDrag";

type Menu =
  | { kind: "edge"; x: number; y: number; edge: LaidEdge }
  | { kind: "canvas"; x: number; y: number; at: Pt }
  | { kind: "card"; x: number; y: number; key: string; at: Pt }
  | { kind: "cancel"; x: number; y: number; key: string };

/// The chain canvas: cards left to right in dependency order, releases as their own nodes. The
/// dependencies can be edited with the mouse: drag from a card's handle onto another card to add
/// one, click a line to select it, drag a selected line's end onto another card to move it,
/// Delete or the right-click menu to remove it. A connector released on empty canvas, a
/// double-click, N and the right-click menus make cards without the dialog. The daemon decides;
/// its refusal shows on top.
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
  const [menu, setMenu] = useState<Menu>();
  const inner = useRef<HTMLDivElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const local = (clientX: number, clientY: number): Pt => {
    const r = inner.current?.getBoundingClientRect();
    return { x: clientX - (r?.left ?? 0), y: clientY - (r?.top ?? 0) };
  };
  const keyOf = (k: string) => label(k);
  const newCardAt = (at: Pt, rel: { after?: string; before?: string } = {}) =>
    dispatch({ type: "openQuickCard", chain: chainId, x: at.x, y: at.y, ...rel });
  const quick = state.quick.card?.chain === chainId ? state.quick.card : undefined;
  const quickLabel = quick?.after ? `New card after ${quick.after}` : quick?.before ? `New card before ${quick.before}` : "New card";
  useCanvasKeys(state, dispatch, () =>
    newCardAt({ x: (root.current?.scrollLeft ?? 0) + 24, y: (root.current?.scrollTop ?? 0) + 48 }));

  const drop = (d: Drag, target: string | undefined, at: Pt) => {
    if (!target) {
      // Released on empty canvas: a new card after the card the line came from.
      if (d.mode === "new") newCardAt(at, { after: keyOf(d.node) });
      return;
    }
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

  const act = (key: string, action: TicketAction) => dispatch({ type: "ticketAction", key, action });
  const entriesOf = (m: Menu): MenuEntry[] => {
    switch (m.kind) {
      case "edge":
        return [{
          label: "Remove dependency", testId: "menu-remove-edge", danger: true,
          run: () => dispatch({ type: "removeEdge", chain: chainId, from: m.edge.from, to: m.edge.to }),
        }];
      case "canvas":
        return canvasEntries({
          newCard: () => newCardAt(m.at),
          newChain: () => dispatch({ type: "openDialog", dialog: { kind: "newChain", project: projectKey } }),
          importPlan: () => dispatch({ type: "openDialog", dialog: { kind: "import", project: projectKey } }),
          askPlanner: () => dispatch({ type: "navigate", route: { name: "planner", project: projectKey } }),
        });
      case "card": {
        // Entries that depend on the card's state wait for the daemon's fresh word on it.
        const fresh = state.quick.peek?.key === m.key && state.quick.peek.fresh;
        return cardEntries(fresh ? state.details[m.key] : undefined, {
          open: () => dispatch({ type: "selectCard", project: projectKey, chain: chain.chain.name, key: m.key }),
          act: (a) => act(m.key, a),
          addAfter: () => newCardAt(m.at, { after: m.key }),
          addBefore: () => newCardAt(m.at, { before: m.key }),
          askCancel: () => setMenu({ kind: "cancel", x: m.x, y: m.y, key: m.key }),
        });
      }
      case "cancel":
        return [];
    }
  };

  const { width, height } = canvasSize(layout.columns, layout.lanes);
  const sel = state.route.name === "card" ? state.route.key : "";
  return (
    <div ref={root} className={`canvas${drag ? " dragging" : ""}`} data-testid="canvas" data-chain={chain.chain.name}>
      {state.edges.error && (
        <div className="edge-error" role="alert" data-testid="edge-error">
          <span>{state.edges.error}</span>
          <button type="button" className="link-btn" onClick={() => dispatch({ type: "dismissEdgeError" })}>Dismiss</button>
        </div>
      )}
      <div
        className="canvas-inner" ref={inner} style={{ width, height }} onClick={() => select(undefined)}
        onDoubleClick={(e) => {
          if (e.target instanceof Element && e.target.closest(".node, .handle, .quick-card, .edge, .end")) return;
          newCardAt(local(e.clientX, e.clientY));
        }}
        onContextMenu={(e: ReactMouseEvent) => {
          if (e.target instanceof Element && e.target.closest(".node, .quick-card")) return;
          e.preventDefault();
          setMenu({ kind: "canvas", x: e.clientX, y: e.clientY, at: local(e.clientX, e.clientY) });
        }}
      >
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
              // A focused card's allowed actions are read, so S can start it.
              onFocus={() => dispatch({ type: "peekCard", key: t.key })}
              onContextMenu={(e) => {
                e.preventDefault();
                e.stopPropagation();
                dispatch({ type: "peekCard", key: t.key });
                setMenu({ kind: "card", x: e.clientX, y: e.clientY, key: t.key, at: local(e.clientX, e.clientY) });
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
            setMenu({ kind: "edge", ...p, edge });
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
        {quick && <QuickCard key={`${quick.x},${quick.y}`} quick={quick} label={quickLabel} state={state} dispatch={dispatch} />}
      </div>
      {menu?.kind === "cancel" && (
        <div className="ctx-menu cancel-pop" style={{ left: Math.max(4, Math.min(menu.x, window.innerWidth - 300)), top: menu.y }}>
          <CancelConfirm
            cardKey={menu.key} onKeep={() => setMenu(undefined)}
            onSend={(text) => {
              setMenu(undefined);
              dispatch({ type: "ticketAction", key: menu.key, action: "cancel", text });
            }}
          />
        </div>
      )}
      {menu && menu.kind !== "cancel" && <ContextMenu x={menu.x} y={menu.y} onClose={() => setMenu(undefined)} entries={entriesOf(menu)} />}
    </div>
  );
}
