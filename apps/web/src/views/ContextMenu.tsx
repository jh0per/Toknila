import { useEffect, useRef } from "react";

export interface MenuEntry {
  label: string;
  testId: string;
  run: () => void;
  danger?: boolean;
}

/// A small menu at the pointer. Escape, a click outside it, scrolling or leaving the window
/// close it; picking an entry closes it and runs the entry.
export function ContextMenu({ x, y, entries, onClose }: {
  x: number;
  y: number;
  entries: MenuEntry[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    const down = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    window.addEventListener("keydown", key);
    window.addEventListener("mousedown", down);
    window.addEventListener("blur", onClose);
    window.addEventListener("resize", onClose);
    ref.current?.querySelector("button")?.focus();
    return () => {
      window.removeEventListener("keydown", key);
      window.removeEventListener("mousedown", down);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);
  // Kept inside the window: a menu opened near the right or bottom edge opens towards the middle.
  const left = Math.min(x, window.innerWidth - 200);
  const top = Math.min(y, window.innerHeight - 40 * entries.length - 8);
  return (
    <div ref={ref} className="ctx-menu" role="menu" data-testid="context-menu" style={{ left: Math.max(4, left), top: Math.max(4, top) }}>
      {entries.map((m) => (
        <button
          key={m.testId}
          type="button"
          role="menuitem"
          data-testid={m.testId}
          className={m.danger ? "danger" : ""}
          onClick={() => {
            onClose();
            m.run();
          }}
        >
          {m.label}
        </button>
      ))}
    </div>
  );
}
