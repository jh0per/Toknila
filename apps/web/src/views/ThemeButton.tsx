import { useCallback, useRef, useState } from "react";
import type { Intent } from "../store/msg";
import { THEMES, THEME_LABEL, type Theme } from "../store/theme";
import { ContextMenu } from "./ContextMenu";

/// The Theme button of the top bar: System, Light or Dark, remembered for this viewer in this
/// browser. System follows the colour scheme of the computer.
export function ThemeButton({ theme, dispatch }: { theme: Theme; dispatch: (i: Intent) => void }) {
  const button = useRef<HTMLButtonElement>(null);
  const [at, setAt] = useState<{ x: number; y: number }>();
  const close = useCallback(() => setAt(undefined), []);
  return (
    <>
      <button
        ref={button} type="button" className="btn text" data-testid="theme-button" data-theme-choice={theme}
        aria-haspopup="menu" aria-expanded={at !== undefined} title={`Colour theme: ${THEME_LABEL[theme]}`}
        onClick={() => {
          const r = button.current?.getBoundingClientRect();
          setAt(at ? undefined : { x: r?.left ?? 0, y: (r?.bottom ?? 0) + 4 });
        }}
        // The menu closes on a press outside it; this press toggles it, so it must not reopen it.
        onMouseDown={(e) => at && e.stopPropagation()}
      >
        Theme
      </button>
      {at && (
        <ContextMenu
          x={at.x} y={at.y} onClose={close} title="Theme"
          entries={THEMES.map((t) => ({
            label: THEME_LABEL[t], testId: `theme-${t}`, checked: t === theme, run: () => dispatch({ type: "setTheme", theme: t }),
          }))}
        />
      )}
    </>
  );
}
