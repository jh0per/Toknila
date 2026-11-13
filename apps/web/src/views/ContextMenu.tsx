import { type KeyboardEvent, useEffect, useLayoutEffect, useRef, useState } from "react";
import { type MenuEntry, moveFocus, shortcutEntry, typeAhead } from "./menuGroups";

export type { MenuEntry } from "./menuGroups";

/// How long the letters typed in a row count as one word for type-ahead.
const TYPING_MS = 700;

/// A menu at the pointer: groups of entries with separators, a title when the menu is about one
/// thing (a line between two cards), and the shortcut of each entry at its right. The keyboard
/// works as in a native menu: Up, Down, Tab, Home and End move, a letter jumps to the entry that
/// starts with it (or runs the entry whose shortcut it is), Enter picks, Escape closes. A click
/// outside it, scrolling or leaving the window close it; picking an entry closes it and runs it.
export function ContextMenu({ x, y, entries, title, onClose }: {
  x: number;
  y: number;
  entries: MenuEntry[];
  title?: string;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const typed = useRef({ text: "", at: 0 });
  const [pos, setPos] = useState({ left: x, top: y });
  // A menu of choices (the theme) marks the current one in a column of its own.
  const choice = entries.some((m) => m.checked !== undefined);
  const buttons = () => Array.from(ref.current?.querySelectorAll<HTMLButtonElement>("button[role^=menuitem]") ?? []);
  const focused = () => buttons().indexOf(document.activeElement as HTMLButtonElement);

  // What had the focus when the menu opened gets it back on Escape.
  const opener = useRef(document.activeElement as HTMLElement | null);
  useEffect(() => {
    const key = (e: globalThis.KeyboardEvent) => {
      if (e.key !== "Escape") return;
      onClose();
      opener.current?.focus?.();
    };
    const down = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    window.addEventListener("keydown", key);
    window.addEventListener("mousedown", down);
    window.addEventListener("blur", onClose);
    window.addEventListener("resize", onClose);
    return () => {
      window.removeEventListener("keydown", key);
      window.removeEventListener("mousedown", down);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  // Kept inside the window by its measured size: a menu opened near the right or bottom edge
  // opens towards the middle. The first entry takes the focus once, then the keys move it.
  const first = useRef(true);
  useLayoutEffect(() => {
    const box = ref.current?.getBoundingClientRect();
    if (box) {
      setPos({
        left: Math.max(4, Math.min(x, window.innerWidth - box.width - 4)),
        top: Math.max(4, Math.min(y, window.innerHeight - box.height - 4)),
      });
    }
    if (first.current) {
      first.current = false;
      buttons()[0]?.focus();
    }
  }, [x, y, entries.length]);

  const pick = (m: MenuEntry) => {
    if (m.disabled) return;
    onClose();
    m.run();
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const list = buttons();
    const to = moveFocus(e.key, focused(), list.length, e.shiftKey);
    if (to !== undefined) {
      e.preventDefault();
      list[to]?.focus();
      return;
    }
    if (e.key.length !== 1 && e.key !== "Delete") return;
    const hit = shortcutEntry(entries, e.key);
    if (hit) {
      e.preventDefault();
      pick(hit);
      return;
    }
    if (e.key.length !== 1 || e.key === " ") return;
    const now = Date.now();
    const text = (now - typed.current.at < TYPING_MS ? typed.current.text : "") + e.key;
    typed.current = { text, at: now };
    const i = typeAhead(entries.map((m) => m.label), text, focused());
    if (i !== undefined) {
      e.preventDefault();
      list[i]?.focus();
    }
  };

  return (
    <div
      ref={ref} className="ctx-menu" role="menu" aria-label={title} data-testid="context-menu"
      style={pos} onKeyDown={onKeyDown}
    >
      {title && <div className="ctx-title" role="presentation" data-testid="context-menu-title">{title}</div>}
      {entries.map((m) => (
        <div key={m.testId} role="none" className="ctx-item">
          {m.sep && <div className="ctx-sep" role="separator" />}
          <button
            type="button" role={choice ? "menuitemradio" : "menuitem"} aria-checked={choice ? Boolean(m.checked) : undefined}
            data-testid={m.testId} title={m.hint} className={m.danger ? "danger" : ""} aria-disabled={m.disabled || undefined} onClick={() => pick(m)}
          >
            {choice && <span className="ctx-check" aria-hidden="true">{m.checked ? "✓" : ""}</span>}
            <span className="ctx-label">{m.label}</span>
            {m.keys && <kbd>{m.keys}</kbd>}
          </button>
        </div>
      ))}
    </div>
  );
}
