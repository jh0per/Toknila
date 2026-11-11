import { type PointerEvent as ReactPointerEvent, type RefObject, useCallback, useEffect, useRef, useState } from "react";
import { type DropBox, dropTarget } from "../store/designBoard";
import type { Ref } from "../store/designs";

/// A handle being dragged from a card's region: whose reference it is and where the pointer is
/// (client pixels). `over` is the card the pointer would drop it on.
export interface RefDrag {
  card: string;
  ref: Ref;
  from: { x: number; y: number };
  at: { x: number; y: number };
  over: string | undefined;
}

/// The screen boxes of everything in `root` that a reference can be dropped on: the elements that
/// carry `data-drop-card`, one per region and one per card chip.
function boxes(root: HTMLElement): DropBox[] {
  return [...root.querySelectorAll<HTMLElement>("[data-drop-card]")].map((el) => {
    const r = el.getBoundingClientRect();
    return { card: el.dataset.dropCard ?? "", x: r.left, y: r.top, w: r.width, h: r.height };
  });
}

/// Dragging a region's handle onto another card. A press that does not move is nothing; a release
/// on another card calls `onDrop`; a release anywhere else, or Escape, cancels.
export function useRefDrag(root: RefObject<HTMLElement>, onDrop: (card: string, ref: Ref, target: string) => void) {
  const [drag, setDrag] = useState<RefDrag>();
  const live = useRef<RefDrag>();
  const stop = useRef<() => void>();
  useEffect(() => () => stop.current?.(), []);

  const begin = useCallback((e: ReactPointerEvent, card: string, ref: Ref) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    stop.current?.();
    const from = { x: e.clientX, y: e.clientY };
    const set = (d: RefDrag | undefined) => {
      live.current = d;
      setDrag(d);
    };
    const move = (ev: PointerEvent) => {
      const at = { x: ev.clientX, y: ev.clientY };
      if (!live.current && Math.hypot(at.x - from.x, at.y - from.y) < 5) return;
      const over = root.current ? dropTarget(boxes(root.current), at, card) : undefined;
      set({ card, ref, from, at, over });
    };
    const end = (ev: PointerEvent) => {
      const d = live.current;
      finish();
      if (ev.type === "pointerup" && d?.over) onDrop(card, ref, d.over);
    };
    const key = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") finish();
    };
    const finish = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      window.removeEventListener("keydown", key);
      stop.current = undefined;
      set(undefined);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
    window.addEventListener("keydown", key);
    stop.current = finish;
  }, [root, onDrop]);

  return { drag, begin };
}
