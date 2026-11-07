import type { PointerEvent as ReactPointerEvent } from "react";
import type { LaidEdge, LaidNode } from "../layout";
import { type Pt, curve, inPoint, outPoint } from "./canvasGeometry";
import type { Drag } from "./useEdgeDrag";

/// The line of a gesture: from where it started to the pointer, or, when the start of a selected
/// line is dragged, from the pointer to the line's fixed end.
const ghost = (d: Drag): string =>
  d.mode === "move" && d.end === "from" ? curve(d.at, d.anchor) : curve(d.anchor, d.at);

/// The lines of the diagram. Each has a wide invisible stroke to click, right-click and select;
/// the selected line shows a handle at each end to drag onto another card (in a second layer
/// above the cards' own handles); a line being dragged is drawn to the pointer.
export function EdgeLayer({ width, height, edges, at, label, selected, drag, onSelect, onMenu, onGrab }: {
  width: number;
  height: number;
  edges: LaidEdge[];
  at: Map<string, LaidNode>;
  /// A node's name for people: a card's key, a milestone's version.
  label: (nodeKey: string) => string;
  selected: LaidEdge | undefined;
  drag: Drag | undefined;
  onSelect: (e: LaidEdge) => void;
  onMenu: (e: LaidEdge, at: Pt) => void;
  onGrab: (e: LaidEdge, end: "from" | "to", ev: ReactPointerEvent) => void;
}) {
  const a = selected && at.get(selected.from);
  const b = selected && at.get(selected.to);
  return (
    <>
      <svg width={width} height={height} aria-hidden="true" data-testid="edges">
        {edges.map((e) => {
          const from = at.get(e.from);
          const to = at.get(e.to);
          if (!from || !to) return null;
          const d = curve(outPoint(from), inPoint(to));
          const sel = selected?.from === e.from && selected.to === e.to;
          const name = `${label(e.from)}>${label(e.to)}`;
          return (
            <g key={`${e.from}>${e.to}`} className={`edge${sel ? " sel" : ""}`} data-edge={name} data-selected={sel}>
              <path className="line" d={d} />
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
        {drag && <path className="ghost" d={ghost(drag)} />}
      </svg>
      <svg className="ends" width={width} height={height} aria-hidden="true">
        {selected && a && b && (
          <>
            <circle
              className="end" data-testid="edge-end-from" cx={outPoint(a).x} cy={outPoint(a).y} r={6}
              onPointerDown={(ev) => onGrab(selected, "from", ev)}
            />
            <circle
              className="end" data-testid="edge-end-to" cx={inPoint(b).x} cy={inPoint(b).y} r={6}
              onPointerDown={(ev) => onGrab(selected, "to", ev)}
            />
          </>
        )}
      </svg>
    </>
  );
}
