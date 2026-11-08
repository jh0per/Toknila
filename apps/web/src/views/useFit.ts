import { type RefObject, useEffect, useState } from "react";

/// The diagram is never drawn smaller than this, so cards stay readable.
export const MIN_ZOOM = 0.4;

/// The zoom at which a diagram of `width` x `height` fits a view of `view`, never above 100%.
export function fitZoom(view: { w: number; h: number }, width: number, height: number): number {
  if (view.w <= 0 || view.h <= 0 || width <= 0 || height <= 0) return 1;
  return Math.max(MIN_ZOOM, Math.min(1, view.w / width, view.h / height));
}

/// Fitting the diagram to its view. `cut` says that at 100% some cards are outside the view; `fit`
/// switches between 100% and the zoom that shows every card (it follows the view while it is on).
/// The view is the scrolling box `scroller`; the diagram is `width` x `height` pixels.
export function useFit(scroller: RefObject<HTMLElement>, width: number, height: number): {
  zoom: number;
  fitted: boolean;
  cut: boolean;
  toggle: () => void;
} {
  const [view, setView] = useState({ w: 0, h: 0 });
  const [fitted, setFitted] = useState(false);
  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const read = () => setView({ w: el.clientWidth, h: el.clientHeight });
    read();
    const watch = new ResizeObserver(read);
    watch.observe(el);
    return () => watch.disconnect();
  }, [scroller]);
  const fit = fitZoom(view, width, height);
  return { zoom: fitted ? fit : 1, fitted, cut: fit < 1, toggle: () => setFitted((f) => !f) };
}
