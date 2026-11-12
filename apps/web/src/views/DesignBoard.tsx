import { useCallback, useEffect, useRef, useState } from "react";
import { get } from "../api/http";
import type { ChainDetail, Design, DesignList } from "../api/types";
import { boardItems, layoutBoard, refIndex } from "../store/designBoard";
import { moveDesign } from "../store/designEffects";
import { type Ref } from "../store/designs";
import type { Intent } from "../store/msg";
import { pinVersion, pinsFor } from "../store/pins";
import { type State, currentProject } from "../store/state";
import { BoardTile } from "./BoardTile";
import { ZoomControls } from "./ZoomControls";
import { usePins } from "./usePins";
import { useFit } from "./useFit";
import { useRefDrag } from "./useRefDrag";

/// The hint of a chain that has no designs.
export const EMPTY_HINT = "No designs yet. Add a design to a story and its mockup shows here.";

const enc = encodeURIComponent;

/// The Designs view of a chain: the mockups of its cards laid out as a board, every card's region
/// outlined on its design in the card's status colour, with the assistant's pins. A region's label
/// opens its card; dragging a region's handle onto another card (a region or a chip of the bar on
/// top) moves the reference there. Zoom, pan (scroll) and Fit are the diagram's.
export function DesignBoard({ chain, state, dispatch }: { chain: ChainDetail; state: State; dispatch: (i: Intent) => void }) {
  const hasDesigns = chain.tickets.some((t) => t.status !== "cancelled" && (t.designs ?? []).length > 0);
  if (hasDesigns) return <BoardView chain={chain} state={state} dispatch={dispatch} />;
  return (
    <div className="canvas-frame board-frame">
      <div className="board-empty" data-testid="designs-empty"><p className="muted">{EMPTY_HINT}</p></div>
    </div>
  );
}

/// The board of a chain that has designs; mounted only then, so its zoom measures its own view.
function BoardView({ chain, state, dispatch }: { chain: ChainDetail; state: State; dispatch: (i: Intent) => void }) {
  const project = currentProject(state);
  const projectKey = project?.key ?? chain.chain.project_id;
  const live = chain.tickets.filter((t) => t.status !== "cancelled");
  const wanted = live.flatMap((t) => (t.designs ?? []).map((r) => r.design));
  const signature = [...new Set(wanted)].sort().join(",");
  const [library, setLibrary] = useState<Design[] | undefined>();
  useEffect(() => {
    let on = true;
    get<DesignList>(`/projects/${enc(projectKey)}/designs`)
      .then((l) => on && setLibrary(l.designs))
      .catch(() => on && setLibrary([]));
    return () => { on = false; };
  }, [projectKey, signature]);
  const { pins } = usePins(projectKey, pinVersion(state.events));
  const [aspects, setAspects] = useState<Record<string, number>>({});
  const [activePin, setActivePin] = useState<string>();
  const [error, setError] = useState<string>();

  const items = library ? boardItems(chain.tickets, library) : [];
  const board = layoutBoard(items.map((i) => i.id), aspects);
  const scroller = useRef<HTMLDivElement>(null);
  const frame = useRef<HTMLDivElement>(null);
  const fit = useFit(scroller, `designs:${chain.chain.id}`, board.width, board.height);

  const route = state.route;
  const current = route.name === "card" ? route.key : undefined;
  const open = (key: string) =>
    dispatch({ type: "navigate", route: { name: "card", project: projectKey, chain: chain.chain.name, key, view: "designs" } });
  const onAspect = useCallback((id: string, a: number) => {
    setAspects((all) => (Math.abs((all[id] ?? 0) - a) < 0.001 ? all : { ...all, [id]: a }));
  }, []);

  // One request: the daemon moves the reference whole or changes nothing.
  const move = useCallback(async (card: string, ref: Ref, target: string) => {
    const from = chain.tickets.find((t) => t.key === card);
    const index = from && refIndex(from.designs ?? [], ref);
    if (!from || index === undefined || !chain.tickets.some((t) => t.key === target)) return;
    setError(undefined);
    try {
      await moveDesign(from.key, index, target);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [chain.tickets]);
  const { drag, begin } = useRefDrag(frame, move);

  return (
    <div className={`canvas-frame board-frame${drag ? " dragging" : ""}`} ref={frame} data-testid="design-board" data-chain={chain.chain.name}>
      <div className="board-cards" data-testid="board-cards" role="group" aria-label="Stories">
        <span className="muted">Stories</span>
        {live.map((t) => (
          <button
            key={t.id} type="button" className={`board-chip s-${t.status}${current === t.key ? " current" : ""}${drag?.over === t.key ? " over" : ""}`}
            data-testid="board-chip" data-card={t.key} data-drop-card={t.key} title={t.title} onClick={() => open(t.key)}
          >
            {t.key}
          </button>
        ))}
        {error && (
          <span className="error" role="alert" data-testid="board-error">
            {error} <button type="button" className="link-btn" onClick={() => setError(undefined)}>Dismiss</button>
          </span>
        )}
      </div>
      <div ref={scroller} className="canvas board-scroll" data-testid="board-scroll">
        {library === undefined ? <p className="muted board-loading">Loading…</p> : (
          <div className="canvas-wrap" style={{ width: board.width * fit.zoom, height: board.height * fit.zoom }}>
            <div
              className="canvas-inner" data-testid="board-inner" data-zoom={fit.zoom}
              style={{ width: board.width, height: board.height, transform: fit.zoom === 1 ? undefined : `scale(${fit.zoom})` }}
            >
              {items.map((item, i) => (
                <BoardTile
                  key={item.id} project={projectKey} item={item} tile={board.tiles[i]} pins={pinsFor(pins, item.id)}
                  activePin={activePin} onPin={(id) => setActivePin(activePin === id ? undefined : id)} onAspect={onAspect}
                  current={current} drag={drag}
                  actions={{ open, grab: (e, card, use, design) => begin(e, card, { design, region: use.region }) }}
                />
              ))}
            </div>
          </div>
        )}
      </div>
      {drag && (
        <svg className="board-drag" data-testid="board-drag" data-over={drag.over ?? ""} aria-hidden="true">
          <line x1={drag.from.x} y1={drag.from.y} x2={drag.at.x} y2={drag.at.y} />
          <circle cx={drag.at.x} cy={drag.at.y} r={5} />
        </svg>
      )}
      <p className="canvas-hint" data-testid="board-hint">Click a label to open its story. Drag a handle onto another story to move the reference.</p>
      <div className="canvas-corner">
        {fit.cut && !fit.fitted && <span className="canvas-cut" data-testid="cut-hint">Some designs are out of view.</span>}
        <ZoomControls
          zoom={fit.zoom} fitted={fit.fitted} cut={fit.cut} onFit={fit.fit} onZoomIn={fit.zoomIn} onZoomOut={fit.zoomOut} what="every design"
        />
        <p className="canvas-legend" data-testid="canvas-legend" title="The colour of a story's status">
          <span className="tone-todo">todo</span>
          <span className="tone-prog">in progress</span>
          <span className="tone-attn">needs you</span>
          <span className="tone-done">done</span>
        </p>
      </div>
    </div>
  );
}
