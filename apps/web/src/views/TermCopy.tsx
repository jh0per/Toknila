import { useRef, useState } from "react";
import { ContextMenu } from "./ContextMenu";

/// The copy controls of a terminal's status line: Copy (the selection, else the visible screen),
/// a menu with Copy all (the whole scrollback), and, when the clipboard refused an agent's write
/// (no user gesture), "Copied text ready" with a button that puts that text on the clipboard.
export function TermCopy({ copy, copyAll, ready, takeReady, copied }: {
  copy: () => void;
  copyAll: () => void;
  /// Text the agent asked to copy that the clipboard refused.
  ready: boolean;
  takeReady: () => void;
  /// A short confirmation of the last copy.
  copied: boolean;
}) {
  const more = useRef<HTMLButtonElement>(null);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const open = () => {
    const r = more.current?.getBoundingClientRect();
    setMenu(r ? { x: r.left, y: r.bottom } : { x: 0, y: 0 });
  };
  return (
    <span className="term-copy">
      {ready && (
        <span data-testid="term-copy-ready">
          <span className="muted">Copied text ready </span>
          <button type="button" data-testid="term-copy-ready-button" onClick={takeReady}>Copy</button>
        </span>
      )}
      {copied && <span className="muted" data-testid="term-copied">copied</span>}
      <button type="button" data-testid="term-copy" title="Copy the selection, or the visible screen" onClick={copy}>Copy</button>
      <button
        type="button" ref={more} data-testid="term-copy-more" aria-label="More copy options" aria-haspopup="menu"
        onClick={open}
      >
        ▾
      </button>
      {menu && (
        <ContextMenu
          x={menu.x} y={menu.y} onClose={() => setMenu(null)}
          entries={[{ label: "Copy all", testId: "term-copy-all", hint: "The whole scrollback the window holds", run: copyAll }]}
        />
      )}
    </span>
  );
}
