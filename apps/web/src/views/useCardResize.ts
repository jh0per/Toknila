import { type MutableRefObject, type PointerEvent as ReactPointerEvent, useEffect, useRef, useState } from "react";
import type { Size } from "../api/types";
import { CLICK_SLOP } from "./useCardDrag";
import { clampSize } from "./cardSize";

/// A card on its way to a new size.
export interface Resizing {
  id: string;
  size: Size;
}

/// Resizing a card with the grip at its bottom-right corner. `begin` is called on pointer down on
/// the grip with the card's id, its size now and `minHeight(w)`, the least height its content
/// needs at width `w`; while the pointer is down `resizing` says the size to draw (on the grid,
/// 196 to 480 wide, from the content's height to 320 tall; `zoom` is the diagram's zoom), and on
/// release `done` gets the final size. Escape puts the card back. The click that ends a resize
/// must not open the card (`wasResize`).
export function useCardResize(
  zoom: MutableRefObject<number>,
  done: (id: string, size: Size) => void,
): {
  resizing: Resizing | undefined;
  begin: (id: string, from: Size, minHeight: (w: number) => number, e: ReactPointerEvent) => void;
  wasResize: () => boolean;
} {
  const [resizing, setResizing] = useState<Resizing>();
  const latest = useRef(done);
  latest.current = done;
  const swallow = useRef(false);
  const stop = useRef<() => void>();
  useEffect(() => () => stop.current?.(), []);

  const begin = (id: string, from: Size, minHeight: (w: number) => number, e: ReactPointerEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    stop.current?.();
    const down = { x: e.clientX, y: e.clientY };
    let far = false;
    let last = from;
    const move = (ev: PointerEvent) => {
      const dx = ev.clientX - down.x;
      const dy = ev.clientY - down.y;
      if (!far && Math.hypot(dx, dy) < CLICK_SLOP) return;
      far = true;
      const raw = { w: from.w + dx / zoom.current, h: from.h + dy / zoom.current };
      last = clampSize(raw, minHeight(clampSize(raw, 0).w));
      setResizing({ id, size: last });
    };
    const end = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("keydown", key);
      stop.current = undefined;
      setResizing(undefined);
    };
    const up = () => {
      end();
      if (!far) return;
      swallow.current = true;
      setTimeout(() => (swallow.current = false), 0);
      latest.current(id, last);
    };
    const key = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") {
        far = false;
        end();
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("keydown", key);
    stop.current = end;
  };

  return { resizing, begin, wasResize: () => swallow.current };
}
