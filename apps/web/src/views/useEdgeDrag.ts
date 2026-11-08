import { type PointerEvent as ReactPointerEvent, type RefObject, useEffect, useRef, useState } from "react";
import type { Pt } from "./canvasGeometry";

/// A line being dragged: a new one from a card's handle, or one end of a selected line.
export type Drag =
  | { mode: "new"; node: string; anchor: Pt; at: Pt; over: string | undefined }
  | { mode: "move"; edge: { from: string; to: string }; end: "from" | "to"; anchor: Pt; at: Pt; over: string | undefined };

type Start = Omit<Extract<Drag, { mode: "new" }>, "at" | "over"> | Omit<Extract<Drag, { mode: "move" }>, "at" | "over">;

/// The pointer gesture of the diagram. `start` is called on pointer down on a handle; while the
/// pointer is down `drag` says where the line ends and which card is under it (`over`); on release
/// over a card `drop` gets the gesture and that card's node key, on release over empty canvas it
/// gets no target and the point (canvas pixels). Escape cancels. The gesture is
/// the only state kept here: the rules are the daemon's.
export function useEdgeDrag(
  inner: RefObject<HTMLElement>,
  drop: (d: Drag, target: string | undefined, at: Pt) => void,
  /// The diagram's zoom: canvas pixels are window pixels divided by it.
  zoom = 1,
): { drag: Drag | undefined; start: (s: Start, e: ReactPointerEvent) => void } {
  const [drag, setDrag] = useState<Drag>();
  const latest = useRef(drop);
  latest.current = drop;
  const current = useRef<Drag>();
  current.current = drag;
  const scale = useRef(zoom);
  scale.current = zoom;
  const active = drag !== undefined;

  const local = (clientX: number, clientY: number): Pt => {
    const r = inner.current?.getBoundingClientRect();
    return { x: (clientX - (r?.left ?? 0)) / scale.current, y: (clientY - (r?.top ?? 0)) / scale.current };
  };

  const start = (s: Start, e: ReactPointerEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    setDrag({ ...s, at: local(e.clientX, e.clientY), over: undefined } as Drag);
  };

  useEffect(() => {
    if (!active) return;
    const nodeAt = (e: PointerEvent) =>
      document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>("[data-node]")?.dataset.node;
    const move = (e: PointerEvent) =>
      setDrag((d) => d && { ...d, at: local(e.clientX, e.clientY), over: nodeAt(e) });
    const up = (e: PointerEvent) => {
      const target = nodeAt(e);
      const d = current.current;
      setDrag(undefined);
      const onCanvas = !!document.elementFromPoint(e.clientX, e.clientY)?.closest(".canvas-wrap");
      if (d && (target || onCanvas)) latest.current(d, target, local(e.clientX, e.clientY));
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") setDrag(undefined);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("keydown", key);
    };
  }, [active]);

  return { drag, start };
}
