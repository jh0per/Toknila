import { type RefObject, useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  MIN_ZOOM, type Scroll, clampZoom, pinchZoom, readStoredZoom, stepZoom, storeZoom, zoomAbout,
} from "./zoomMath";

export { MIN_ZOOM };

/// The zoom at which a diagram of `width` x `height` fits a view of `view`, never above 100%.
export function fitZoom(view: { w: number; h: number }, width: number, height: number): number {
  if (view.w <= 0 || view.h <= 0 || width <= 0 || height <= 0) return 1;
  return Math.max(MIN_ZOOM, Math.min(1, view.w / width, view.h / height));
}

/// The zoom of the diagram. It is free between 40% and 200%: a pinch on the trackpad (a `wheel`
/// event with `ctrlKey` in Chromium and WKWebView, `gesture*` events in WebKit), Cmd/Ctrl with
/// `=`, `-` and `0`, and the buttons, all around the pointer (or the view's centre). `fit` sets
/// the zoom that shows every card and keeps it while the view is resized, until the person zooms
/// by hand. The last zoom of a chain is remembered for this viewer. `cut` says that at the
/// current zoom some cards are outside the view. The view is the scrolling box `scroller`; the
/// diagram is `width` x `height` pixels at 100%.
export function useFit(scroller: RefObject<HTMLElement>, chain: string, width: number, height: number): {
  zoom: number;
  fitted: boolean;
  cut: boolean;
  fit: () => void;
  zoomIn: () => void;
  zoomOut: () => void;
  reset: () => void;
} {
  const [view, setView] = useState({ w: 0, h: 0 });
  const [own, setOwn] = useState<{ chain: string; zoom: number; fitted: boolean }>();
  // The state of another chain is not this chain's: start from what was remembered.
  const now = own?.chain === chain ? own : { chain, zoom: readStoredZoom(chain) ?? 1, fitted: false };
  const fitValue = fitZoom(view, width, height);
  const zoom = now.fitted ? fitValue : now.zoom;
  // What the handlers see before React has drawn again (a pinch sends many events per frame).
  const live = useRef({ zoom, chain });
  live.current = { zoom, chain };
  const pending = useRef<Scroll>();

  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const read = () => setView({ w: el.clientWidth, h: el.clientHeight });
    read();
    const watch = new ResizeObserver(read);
    watch.observe(el);
    return () => watch.disconnect();
  }, [scroller]);

  // The scroll offsets can only be set once the diagram has its new size.
  useLayoutEffect(() => {
    const el = scroller.current;
    const p = pending.current;
    pending.current = undefined;
    if (el && p) {
      el.scrollLeft = p.left;
      el.scrollTop = p.top;
    }
  });

  /// Zooms to `to` keeping the diagram point at `at` (client pixels; the view's centre when
  /// omitted) where it is.
  const zoomTo = useCallback((to: number, at?: { x: number; y: number }) => {
    const el = scroller.current;
    const from = live.current.zoom;
    const next = clampZoom(to);
    if (!el || next === from) return;
    const r = el.getBoundingClientRect();
    const pointer = at ? { x: at.x - r.left, y: at.y - r.top } : { x: el.clientWidth / 2, y: el.clientHeight / 2 };
    pending.current = zoomAbout(pending.current ?? { left: el.scrollLeft, top: el.scrollTop }, pointer, from, next);
    live.current.zoom = next;
    storeZoom(live.current.chain, next);
    setOwn({ chain: live.current.chain, zoom: next, fitted: false });
  }, [scroller]);
  const zoomToRef = useRef(zoomTo);
  zoomToRef.current = zoomTo;

  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const wheel = (ev: WheelEvent) => {
      if (!ev.ctrlKey) return;
      ev.preventDefault();
      zoomToRef.current(pinchZoom(live.current.zoom, ev.deltaY), { x: ev.clientX, y: ev.clientY });
    };
    let base = 1;
    const gestureStart = (ev: Event) => {
      ev.preventDefault();
      base = live.current.zoom;
    };
    const gestureChange = (ev: Event) => {
      ev.preventDefault();
      const g = ev as Event & { scale: number; clientX: number; clientY: number };
      zoomToRef.current(base * g.scale, { x: g.clientX, y: g.clientY });
    };
    el.addEventListener("wheel", wheel, { passive: false });
    el.addEventListener("gesturestart", gestureStart);
    el.addEventListener("gesturechange", gestureChange);
    el.addEventListener("gestureend", gestureStart);
    return () => {
      el.removeEventListener("wheel", wheel);
      el.removeEventListener("gesturestart", gestureStart);
      el.removeEventListener("gesturechange", gestureChange);
      el.removeEventListener("gestureend", gestureStart);
    };
  }, [scroller]);

  const zoomIn = useCallback(() => zoomToRef.current(stepZoom(live.current.zoom, 1)), []);
  const zoomOut = useCallback(() => zoomToRef.current(stepZoom(live.current.zoom, -1)), []);
  const reset = useCallback(() => {
    // Also when fitting already is 100%: the person has taken the zoom into their hands.
    storeZoom(live.current.chain, 1);
    setOwn({ chain: live.current.chain, zoom: 1, fitted: false });
    zoomToRef.current(1);
  }, []);
  useEffect(() => {
    const key = (ev: KeyboardEvent) => {
      if (!(ev.metaKey || ev.ctrlKey) || ev.altKey) return;
      if (ev.key === "=" || ev.key === "+") zoomIn();
      else if (ev.key === "-" || ev.key === "_") zoomOut();
      else if (ev.key === "0") reset();
      else return;
      ev.preventDefault();
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [zoomIn, zoomOut, reset]);

  const fit = () => {
    if (now.fitted) return reset();
    pending.current = undefined;
    setOwn({ chain, zoom: now.zoom, fitted: true });
  };
  return { zoom, fitted: now.fitted, cut: fitValue < zoom - 0.005, fit, zoomIn, zoomOut, reset };
}
