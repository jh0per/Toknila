import { type KeyboardEvent, type PointerEvent, useLayoutEffect, useRef, useState } from "react";
import { DIVIDER_PX, KEY_STEP_PX, clampPanel, readStoredHeight, storeHeight } from "../panelSize";

/// The handle between the diagram and the bottom panel. Drag it (or use the arrow keys) to change
/// the panel's height; the height is remembered for this viewer. It sets `--panel-h` on its parent
/// (the grid of the chain layout); the terminal follows the panel's box by itself.
export function PanelDivider() {
  const handle = useRef<HTMLDivElement>(null);
  const drag = useRef<{ grab: number } | null>(null);
  const [height, setHeight] = useState(0);

  const area = (): HTMLElement | null => handle.current?.parentElement ?? null;
  // The room for the diagram, the divider and the panel: the parent less its top bar.
  const available = (main: HTMLElement) => main.clientHeight - ((main.firstElementChild as HTMLElement | null)?.offsetHeight ?? 0);
  const apply = (px: number, keep: boolean) => {
    const main = area();
    if (!main) return;
    const clamped = clampPanel(px, available(main));
    main.style.setProperty("--panel-h", `${clamped}px`);
    setHeight(clamped);
    if (keep) storeHeight(clamped);
  };
  const current = () => area()?.querySelector<HTMLElement>(".panel")?.getBoundingClientRect().height ?? 0;

  useLayoutEffect(() => {
    const stored = readStoredHeight();
    if (stored !== null) apply(stored, false);
    else setHeight(Math.round(current()));
    // A smaller window can no longer hold the remembered height: keep it inside the minimums.
    const onResize = () => {
      if (area()?.style.getPropertyValue("--panel-h")) apply(Number.parseFloat(area()?.style.getPropertyValue("--panel-h") ?? "0"), false);
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
    // Once per mount: `apply` only reads refs.
  }, []);

  const move = (e: PointerEvent) => {
    const main = area();
    if (!drag.current || !main) return;
    const top = e.clientY - drag.current.grab;
    apply(main.getBoundingClientRect().bottom - (top + DIVIDER_PX), false);
  };
  const down = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { grab: e.clientY - e.currentTarget.getBoundingClientRect().top };
    e.preventDefault();
  };
  const up = () => {
    if (!drag.current) return;
    drag.current = null;
    const main = area();
    if (main) storeHeight(Number.parseFloat(main.style.getPropertyValue("--panel-h")) || current());
  };
  const key = (e: KeyboardEvent) => {
    const step = e.key === "ArrowUp" ? KEY_STEP_PX : e.key === "ArrowDown" ? -KEY_STEP_PX : 0;
    if (!step) return;
    e.preventDefault();
    apply(current() + step, true);
  };

  return (
    <div
      ref={handle} className="divider" role="separator" aria-orientation="horizontal" tabIndex={0}
      aria-label="Resize the panel" aria-valuenow={height} data-testid="panel-divider" title="Drag to resize the panel"
      onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={up} onKeyDown={key}
    />
  );
}
