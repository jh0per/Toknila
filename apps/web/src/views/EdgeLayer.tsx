import type { PointerEvent as ReactPointerEvent } from "react";
import type { LaidEdge } from "../layout";
import { type EdgeGeo, type Pt, curve, edgeId } from "./canvasGeometry";
import type { Drag } from "./useEdgeDrag";

/// The line of a gesture: from where it started to the pointer, or, when the start of a selected
/// line is dragged, from the pointer to the line's fixed end.
const ghost = (d: Drag): string =>
  d.mode === "move" && d.end === "from" ? curve(d.at, d.anchor) : curve(d.anchor, d.at);

/// An arrowhead for the line's state; the tip sits on the card the line enters.
function Arrow({ id }: { id: string }) {
  return (
    <marker id={id} className={id} viewBox="0 0 8 8" refX="7" refY="4" markerWidth="8" markerHeight="8" markerUnits="userSpaceOnUse" orient="auto">
      <path d="M0,0L8,4L0,8z" />
    </marker>
  );
}

/// The lines of the diagram. Each has a wide invisible stroke to click, right-click and select;
/// the selected line shows a handle at each end to drag onto another card (in a second layer
/// above the cards' own handles); a line being dragged is drawn to the pointer. While a card is
/// the focus, its own lines are drawn strong and the others fade (`data-focus`).
export function EdgeLayer({ width, height, edges, implicit, geo, label, selected, focus, drag, onSelect, onMenu, onGrab }: {
  width: number;
  height: number;
  edges: LaidEdge[];
  /// The lines from Start and to the end node: drawn thin and dim, not stored, not selectable.
  implicit: LaidEdge[];
  geo: Map<string, EdgeGeo>;
  /// A node's name for people: a card's key, a milestone's version.
  label: (nodeKey: string) => string;
  selected: LaidEdge | undefined;
  /// The node key of the card in focus, if any.
  focus: string | undefined;
  drag: Drag | undefined;
  onSelect: (e: LaidEdge) => void;
  onMenu: (e: LaidEdge, at: Pt) => void;
  onGrab: (e: LaidEdge, end: "from" | "to", ev: ReactPointerEvent) => void;
}) {
  const sg = selected && geo.get(edgeId(selected));
  return (
    <>
      <svg width={width} height={height} aria-hidden="true" data-testid="edges">
        <defs>
          <Arrow id="arrow" />
          <Arrow id="arrow-hl" />
          <Arrow id="arrow-sel" />
          <Arrow id="arrow-imp" />
        </defs>
        {edges.map((e) => {
          const g = geo.get(edgeId(e));
          if (!g) return null;
          const d = curve(g.from, g.to, g.turn);
          const sel = selected?.from === e.from && selected.to === e.to;
          const mine = focus !== undefined && (e.from === focus || e.to === focus);
          const state = sel ? "sel" : mine ? "hl" : focus !== undefined ? "dim" : "";
          const name = `${label(e.from)}>${label(e.to)}`;
          return (
            <g
              key={edgeId(e)} className={`edge${sel ? " sel" : ""}${state === "hl" ? " hl" : ""}${state === "dim" ? " dim" : ""}`}
              data-edge={name} data-selected={sel} data-focus={state}
            >
              <path className="line" d={d} markerEnd={`url(#${sel ? "arrow-sel" : mine ? "arrow-hl" : "arrow"})`} />
              <path
                className="hit" d={d} data-testid="edge-hit" data-edge={name}
                onClick={(ev) => {
                  ev.stopPropagation();
                  onSelect(e);
                }}
                onContextMenu={(ev) => {
                  ev.preventDefault();
                  ev.stopPropagation();
                  onMenu(e, { x: ev.clientX, y: ev.clientY });
                }}
              />
            </g>
          );
        })}
        {implicit.map((e) => {
          const g = geo.get(edgeId(e));
          if (!g) return null;
          return (
            <g key={edgeId(e)} className="edge implicit" data-implicit={`${label(e.from)}>${label(e.to)}`}>
              <path className="line" d={curve(g.from, g.to, g.turn)} markerEnd="url(#arrow-imp)" />
            </g>
          );
        })}
        {drag && <path className="ghost" d={ghost(drag)} />}
      </svg>
      <svg className="ends" width={width} height={height} aria-hidden="true">
        {selected && sg && (
          <>
            <circle
              className="end" data-testid="edge-end-from" cx={sg.from.x} cy={sg.from.y} r={6}
              onPointerDown={(ev) => onGrab(selected, "from", ev)}
            />
            <circle
              className="end" data-testid="edge-end-to" cx={sg.to.x} cy={sg.to.y} r={6}
              onPointerDown={(ev) => onGrab(selected, "to", ev)}
            />
          </>
        )}
      </svg>
    </>
  );
}
