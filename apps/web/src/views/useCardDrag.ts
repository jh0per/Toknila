import { type MutableRefObject, type PointerEvent as ReactPointerEvent, useEffect, useRef, useState } from "react";
import { type Pt, snapPos } from "./canvasGeometry";
import { dropTarget } from "./epicRows";

/// A press that moves less than this (window pixels) is still a click.
export const CLICK_SLOP = 5;

/// A card on its way to a new place.
export interface Moving {
  id: string;
  pos: Pt;
}

/// Where a card may be dropped besides the diagram: the epic rows of the sidebar. `project` is the
/// card's project key and `own` its epic's id; `over` says which row (a chain id) the card is
/// over while it is dragged, `drop` gets the row it was released on.
export interface EpicDrop {
  project: string;
  own: string;
  over: (chain: string | undefined) => void;
  drop: (id: string, chain: string) => void;
}

/// Dragging a card by its body. `press` is called on pointer down on a card with the id and the
/// place it is drawn at; once the pointer has gone more than a few pixels `moving` says where the
/// card is drawn (snapped to the grid; `zoom` is the diagram's current zoom) and on release `drop` gets the
/// final place. A press that never gets that far is left to the click, and a drag swallows the
/// click that follows it (`wasDrag`). Escape puts the card back. Buttons on a card keep their own
/// clicks. A card released over an epic's row of the sidebar (`epics`) is not put anywhere: the
/// epic goes to `epics.drop` instead.
export function useCardDrag(
  zoom: MutableRefObject<number>,
  drop: (id: string, pos: Pt) => void,
  epics?: EpicDrop,
): { moving: Moving | undefined; press: (id: string, at: Pt, e: ReactPointerEvent) => void; wasDrag: () => boolean } {
  const [moving, setMoving] = useState<Moving>();
  const latest = useRef(drop);
  latest.current = drop;
  const swallow = useRef(false);
  const stop = useRef<() => void>();
  useEffect(() => () => stop.current?.(), []);

  const press = (id: string, at: Pt, e: ReactPointerEvent) => {
    if (e.button !== 0 || (e.target as Element).closest("button, input, textarea, .pr, .grip")) return;
    stop.current?.();
    const down = { x: e.clientX, y: e.clientY };
    let far = false;
    let last = at;
    let over: string | undefined;
    const move = (ev: PointerEvent) => {
      const dx = ev.clientX - down.x;
      const dy = ev.clientY - down.y;
      if (!far && Math.hypot(dx, dy) < CLICK_SLOP) return;
      far = true;
      last = snapPos({ x: at.x + dx / zoom.current, y: at.y + dy / zoom.current });
      setMoving({ id, pos: last });
      if (epics) {
        const row = dropTarget(document.elementFromPoint(ev.clientX, ev.clientY), epics.project, epics.own);
        if (row !== over) epics.over((over = row));
      }
    };
    const end = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("keydown", key);
      stop.current = undefined;
      setMoving(undefined);
      if (over) epics?.over(undefined);
    };
    const up = () => {
      const target = over;
      end();
      if (!far) return;
      // The click that ends this press comes right after, and must not open the card.
      swallow.current = true;
      setTimeout(() => (swallow.current = false), 0);
      if (target && epics) epics.drop(id, target);
      else latest.current(id, last);
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

  return { moving, press, wasDrag: () => swallow.current };
}
