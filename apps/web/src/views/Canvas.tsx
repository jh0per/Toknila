import { type MouseEvent as ReactMouseEvent, type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import type { ChainDetail, Pos, Size, Ticket, TicketAction } from "../api/types";
import { FINISH, START, sceneOf } from "../canvasScene";
import { type LaidEdge, nodeKey } from "../layout";
import { type EdgeRef, idOf } from "../store/edges";
import { epicPanel } from "../store/epic";
import { canRelease } from "../store/release";
import type { Intent } from "../store/msg";
import { type State, currentProject, hrefOf, queuePosition } from "../store/state";
import { epicTitle } from "../store/title";
import { ContextMenu, type MenuEntry } from "./ContextMenu";
import { EdgeLayer } from "./EdgeLayer";
import { QuickCard } from "./QuickCard";
import { type Pt, canvasSize, edgeGeometry, edgeId, outPoint, snapPos } from "./canvasGeometry";
import { cardLayouts, contentOf } from "./cardBoxes";
import { minHeight } from "./cardSize";
import { measureText } from "./measureText";
import { useCardResize } from "./useCardResize";
import { CancelConfirm } from "./CancelConfirm";
import { CanvasChrome } from "./CanvasChrome";
import { FinishNode, ReleaseNode, StartNode, TicketNode } from "./CanvasNodes";
import { ReasonAsk } from "./ReasonAsk";
import { canvasEntries, cardEntries, edgeMenu } from "./canvasMenus";
import { useCanvasKeys } from "./useCanvasKeys";
import { useCardDrag } from "./useCardDrag";
import { type Drag, useEdgeDrag } from "./useEdgeDrag";
import { useFit } from "./useFit";

type Menu =
  | { kind: "edge"; x: number; y: number; edge: LaidEdge }
  | { kind: "canvas"; x: number; y: number; at: Pt }
  | { kind: "card"; x: number; y: number; key: string; at: Pt }
  | { kind: "cancel"; x: number; y: number; key: string }
  | { kind: "ask"; x: number; y: number; key: string; action: "reject" | "return" };

/// The chain canvas: cards left to right in dependency order, releases as their own nodes. The
/// dependencies can be edited with the mouse: drag from a card's handle onto another card to add
/// one, click a line to select it, drag a selected line's end onto another card to move it,
/// Delete or the right-click menu to remove it. A connector released on empty canvas, a
/// double-click, N and the right-click menus make cards without the dialog. The daemon decides;
/// its refusal shows on top. The open card's lines are drawn strong and the others fade; Fit zooms
/// the whole diagram into view.
export function Canvas({ chain, state, dispatch, onEmpty }: {
  chain: ChainDetail;
  state: State;
  dispatch: (i: Intent) => void;
  /// Shown over the diagram while the chain has no cards; the canvas under it keeps working.
  onEmpty?: ReactNode;
}) {
  const chainId = chain.chain.id;
  const project = currentProject(state);
  const projectKey = project?.key ?? chain.chain.project_id;
  const tickets = new Map(chain.tickets.map((t) => [t.id, t]));
  const releases = new Map(chain.releases.map((r) => [r.id, r]));
  const place = (id: string, pos: Pos | null) => {
    const t = tickets.get(id);
    if (t) dispatch({ type: "placeCard", chain: chainId, ticket: id, key: t.key, pos });
  };
  // A card is drawn where it is being dragged, else where it was just put, else where it was saved.
  const zoomRef = useRef(1);
  const { moving, press, wasDrag } = useCardDrag(zoomRef, place);
  const resize = (id: string, size: Size | null) => {
    const t = tickets.get(id);
    if (t) dispatch({ type: "sizeCard", chain: chainId, ticket: id, key: t.key, size });
  };
  const { resizing, begin, wasResize } = useCardResize(zoomRef, resize);
  const pending = state.placement.pending;
  const posOf = (id: string): Pos | undefined =>
    moving?.id === id ? moving.pos : id in pending ? pending[id] ?? undefined : tickets.get(id)?.pos ?? undefined;
  // A card has the size it is being resized to, else the one it was just given, else its own, else
  // the one its content makes.
  const sizes = state.placement.sizes;
  const sizeOf = (t: Ticket): Size | null | undefined =>
    resizing?.id === t.id ? resizing.size : t.id in sizes ? sizes[t.id] : t.size;
  const layouts = cardLayouts(chain, state, sizeOf, measureText);
  const scene = sceneOf(chain, posOf, (id) => {
    const l = layouts.get(id);
    const t = tickets.get(id);
    return l && t ? { w: l.w, h: l.h, own: Boolean(sizeOf(t)) } : undefined;
  });
  const endKey = scene.nodes[scene.nodes.length - 1].key;
  const at = new Map(scene.nodes.map((n) => [n.key, n.box]));
  const geo = edgeGeometry(scene.lines, at);
  const label = (k: string) => {
    if (k === START) return "Start";
    if (k === FINISH) return "Finish";
    const id = idOf(k);
    return tickets.get(id)?.key ?? releases.get(id)?.version ?? id;
  };
  const selected: LaidEdge | undefined = scene.edges.find((e) => {
    const s = state.edges.selected;
    return s?.chain === chainId && s.from === e.from && s.to === e.to;
  });
  const select = (e: LaidEdge | undefined) =>
    dispatch({ type: "selectEdge", edge: e && { chain: chainId, from: e.from, to: e.to } satisfies EdgeRef });
  const [menu, setMenu] = useState<Menu>();
  // "Add dependency": the node key of the card that waits, until the card it waits for is clicked.
  const [linking, setLinking] = useState<string>();
  const [hover, setHover] = useState<string>();
  const inner = useRef<HTMLDivElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const { width, height } = canvasSize(scene.nodes.map((n) => n.box));
  const fit = useFit(root, chainId, width, height);
  zoomRef.current = fit.zoom;
  const local = (clientX: number, clientY: number): Pt => {
    const r = inner.current?.getBoundingClientRect();
    return { x: (clientX - (r?.left ?? 0)) / fit.zoom, y: (clientY - (r?.top ?? 0)) / fit.zoom };
  };
  const newCardAt = (at: Pt, rel: { after?: string; before?: string } = {}) =>
    dispatch({ type: "openQuickCard", chain: chainId, x: at.x, y: at.y, ...rel });
  const quick = state.quick.card?.chain === chainId ? state.quick.card : undefined;
  const quickLabel = quick?.after
    ? `New story that waits for ${quick.after}`
    : quick?.before ? `New story that ${quick.before} waits for` : "New story";
  const sel = state.route.name === "card" ? state.route.key : "";
  const focusTicket = chain.tickets.find((t) => t.key === sel);
  const focusNode = focusTicket ? nodeKey({ kind: "ticket", id: focusTicket.id }) : undefined;
  const nudge = (dx: number, dy: number) => {
    const b = focusNode && at.get(focusNode);
    if (focusTicket && b) place(focusTicket.id, snapPos({ x: b.x + dx, y: b.y + dy }));
  };
  useCanvasKeys(state, dispatch, () =>
    newCardAt({ x: (root.current?.scrollLeft ?? 0) / fit.zoom + 24, y: (root.current?.scrollTop ?? 0) / fit.zoom + 48 }), nudge);

  const drop = (d: Drag, target: string | undefined, at: Pt) => {
    if (!target) {
      // Released on empty canvas: a new card after the card the line came from.
      if (d.mode === "new") newCardAt(at, { after: label(d.node) });
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
  const { drag, start } = useEdgeDrag(inner, drop, fit.zoom);

  // Delete or Backspace removes the selected line, Escape lets go of it (or of "Add dependency");
  // typing in a field or in a menu is left alone.
  useEffect(() => {
    if (!selected && !linking) return;
    const key = (ev: KeyboardEvent) => {
      const t = ev.target as HTMLElement | null;
      if (t?.closest("input, textarea, select, [contenteditable=true], [role=dialog], .ctx-menu")) return;
      if (selected && (ev.key === "Delete" || ev.key === "Backspace")) {
        ev.preventDefault();
        dispatch({ type: "removeEdge", chain: chainId, from: selected.from, to: selected.to });
      } else if (ev.key === "Escape") {
        setLinking(undefined);
        select(undefined);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  const act = (key: string, action: TicketAction, text?: string) => dispatch({ type: "ticketAction", key, action, text });
  const openCard = (key: string) => dispatch({ type: "selectCard", project: projectKey, chain: chain.chain.name, key });
  const entriesOf = (m: Menu): { entries: MenuEntry[]; title?: string } => {
    switch (m.kind) {
      case "edge":
        return edgeMenu(label(m.edge.from), label(m.edge.to), () =>
          dispatch({ type: "removeEdge", chain: chainId, from: m.edge.from, to: m.edge.to }));
      case "canvas":
        return {
          entries: canvasEntries({
            newCard: () => newCardAt(m.at),
            newChain: () => dispatch({ type: "openDialog", dialog: { kind: "newChain", project: projectKey } }),
            importPlan: () => dispatch({ type: "openDialog", dialog: { kind: "import", project: projectKey } }),
            askPlanner: () => dispatch({ type: "openAssistant" }),
          }),
        };
      case "card": {
        // Entries that depend on the card's state wait for the daemon's fresh word on it.
        const fresh = state.quick.peek?.key === m.key && state.quick.peek.fresh;
        const ticket = chain.tickets.find((t) => t.key === m.key);
        return {
          title: m.key,
          entries: cardEntries(fresh ? state.details[m.key] : undefined, {
            open: () => openCard(m.key),
            openTerminal: () => {
              openCard(m.key);
              dispatch({ type: "selectTab", tab: "terminal" });
            },
            edit: () => {
              openCard(m.key);
              dispatch({ type: "editCard", key: m.key });
            },
            act: (a) => act(m.key, a),
            ask: (action) => setMenu({ kind: "ask", x: m.x, y: m.y, key: m.key, action }),
            addAfter: () => newCardAt(m.at, { after: m.key }),
            addBefore: () => newCardAt(m.at, { before: m.key }),
            addDependency: () => ticket && setLinking(nodeKey({ kind: "ticket", id: ticket.id })),
            askCancel: () => setMenu({ kind: "cancel", x: m.x, y: m.y, key: m.key }),
          }),
        };
      }
      case "cancel":
      case "ask":
        return { entries: [] };
    }
  };

  // A card's connector shows while the pointer is on the card, while it is the open card, and
  // while a line is dragged from it.
  const handleShown = (k: string) => hover === k || focusNode === k || (drag?.mode === "new" && drag.node === k);
  const closeMenu = useCallback(() => setMenu(undefined), []);
  const popAt = (m: { x: number }) => Math.max(4, Math.min(m.x, window.innerWidth - 300));
  const linkingKey = linking ? label(linking) : undefined;
  const pick = (id: string) => {
    if (!linking) return false;
    const target = nodeKey({ kind: "ticket", id });
    if (target !== linking) dispatch({ type: "addEdge", chain: chainId, from: target, to: linking });
    setLinking(undefined);
    return true;
  };
  return (
    <div className="canvas-frame">
      <div ref={root} className={`canvas${drag ? " dragging" : ""}${linking ? " linking" : ""}`} data-testid="canvas" data-chain={chain.chain.name}>
        {state.edges.error && (
          <div className="edge-error" role="alert" data-testid="edge-error">
            <span>{state.edges.error}</span>
            <button type="button" className="link-btn" onClick={() => dispatch({ type: "dismissEdgeError" })}>Dismiss</button>
          </div>
        )}
        <div
          className="canvas-wrap" style={{ width: width * fit.zoom, height: height * fit.zoom }} onClick={() => select(undefined)}
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
          <div
            className="canvas-inner" ref={inner} data-zoom={fit.zoom}
            style={{ width, height, transform: fit.zoom === 1 ? undefined : `scale(${fit.zoom})` }}
          >
            {scene.nodes.map((n) => {
              const style = { left: n.box.x, top: n.box.y, width: n.box.w, height: n.box.h };
              const panel = epicPanel(state);
              const open = (kind: "epic" | "release") => () => dispatch({ type: "openEpic", chain: chainId, kind });
              if (n.kind === "start") {
                return <StartNode key={n.key} name={epicTitle(chain)} goal={chain.chain.goal} style={style} selected={panel?.kind === "epic"} onOpen={open("epic")} />;
              }
              // Every card merged: the end node offers Release.
              const onRelease = canRelease(chain) ? () => dispatch({ type: "openRelease", chain: chainId }) : undefined;
              if (n.kind === "finish") return <FinishNode key={n.key} style={style} summary={chain.summary} selected={panel?.kind === "release"} onRelease={onRelease} onOpen={open("release")} />;
              const over = drag?.over === n.key;
              const r = n.kind === "release" ? releases.get(n.id) : undefined;
              const isEnd = n.key === endKey;
              if (r && isEnd) {
                return <FinishNode key={n.key} style={style} summary={chain.summary} release={r} nodeKey={n.key} over={over} selected={panel?.kind === "release"} onRelease={onRelease} onOpen={open("release")} />;
              }
              if (r) return <ReleaseNode key={n.key} release={r} nodeKey={n.key} style={style} over={over} end={isEnd} onRelease={isEnd ? onRelease : undefined} />;
              const t = tickets.get(n.id);
              const layout = layouts.get(n.id);
              if (!t || !layout) return null;
              const route = { name: "card" as const, project: projectKey, chain: chain.chain.name, key: t.key };
              return (
                <TicketNode
                  key={n.key} ticket={t} projectKey={projectKey} layout={layout} href={hrefOf(route)} style={style} selected={sel === t.key} moving={moving?.id === t.id} over={over}
                  linking={Boolean(linking) && n.key !== linking} detail={state.details[t.key]}
                  usage={chain.usage?.[t.key]} runTime={chain.run_time?.[t.key]} queued={queuePosition(state, t.id)} defaultProfile={project?.default_profile}
                  onHover={(on) => setHover(on ? n.key : undefined)}
                  handlers={{
                    activate: () => {
                      if (wasDrag() || wasResize()) return;
                      if (!pick(t.id)) openCard(t.key);
                    },
                    press: (e) => press(t.id, n.box, e),
                    grip: (e) =>
                      begin(t.id, { w: n.box.w, h: n.box.h }, (w) => minHeight(contentOf(t, state.details[t.key], chain.usage?.[t.key] ?? chain.run_time?.[t.key], queuePosition(state, t.id)), w, measureText), e),
                    fit: () => resize(t.id, null),
                    focus: () => dispatch({ type: "peekCard", key: t.key }),
                    menu: (e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      dispatch({ type: "peekCard", key: t.key });
                      setMenu({ kind: "card", x: e.clientX, y: e.clientY, key: t.key, at: local(e.clientX, e.clientY) });
                    },
                    approve: () => act(t.key, "approve"),
                    reject: (p) => setMenu({ kind: "ask", x: p.x, y: p.y, key: t.key, action: "reject" }),
                  }}
                />
              );
            })}
            {scene.nodes.filter((n) => n.kind === "ticket" || n.kind === "release").map((n) => (
              <span
                key={`h-${n.key}`} className={`handle${handleShown(n.key) ? " show" : ""}`} data-node={n.key}
                data-testid={`handle-${label(n.key)}`} title="Drag onto another story to make it wait for this one"
                style={{ left: outPoint(n.box).x - 6, top: outPoint(n.box).y - 6 }}
                onPointerEnter={() => setHover(n.key)}
                onPointerLeave={() => setHover(undefined)}
                onPointerDown={(e) => start({ mode: "new", node: n.key, anchor: outPoint(n.box) }, e)}
              />
            ))}
            <EdgeLayer
              width={width} height={height} edges={scene.edges} implicit={scene.lines.filter((l) => l.implicit)} geo={geo} label={label} selected={selected} focus={focusNode} drag={drag}
              onSelect={select}
              onMenu={(edge, p) => {
                select(edge);
                setMenu({ kind: "edge", ...p, edge });
              }}
              onGrab={(e, end, ev) => {
                const g = geo.get(edgeId(e));
                if (!g) return;
                // The end that stays: the other one.
                start({ mode: "move", edge: { from: e.from, to: e.to }, end, anchor: end === "from" ? g.to : g.from }, ev);
              }}
            />
            {quick && <QuickCard key={`${quick.x},${quick.y}`} quick={quick} label={quickLabel} state={state} dispatch={dispatch} />}
          </div>
        </div>
      </div>
      {onEmpty && chain.tickets.length === 0 && <div className="canvas-empty" data-testid="chain-empty">{onEmpty}</div>}
      <CanvasChrome
        zoom={fit.zoom} fitted={fit.fitted} cut={fit.cut} onFit={fit.fit} onZoomIn={fit.zoomIn} onZoomOut={fit.zoomOut}
        arrangeable={scene.nodes.some((n) => n.placed || n.sized)}
        onArrange={() => dispatch({ type: "arrangeChain", chain: chainId })} linking={linkingKey} onCancelLinking={() => setLinking(undefined)} />
      {menu?.kind === "cancel" && (
        <div className="ctx-menu cancel-pop" style={{ left: popAt(menu), top: menu.y }}>
          <CancelConfirm
            cardKey={menu.key} onKeep={() => setMenu(undefined)}
            onSend={(text) => {
              setMenu(undefined);
              act(menu.key, "cancel", text);
            }}
          />
        </div>
      )}
      {menu?.kind === "ask" && (
        <div className="ctx-menu cancel-pop" style={{ left: popAt(menu), top: menu.y }}>
          <ReasonAsk
            cardKey={menu.key} action={menu.action} onKeep={() => setMenu(undefined)}
            onSend={(text) => {
              setMenu(undefined);
              act(menu.key, menu.action, text);
            }}
          />
        </div>
      )}
      {menu && menu.kind !== "cancel" && menu.kind !== "ask" && (
        <ContextMenu x={menu.x} y={menu.y} onClose={closeMenu} {...entriesOf(menu)} />
      )}
    </div>
  );
}
