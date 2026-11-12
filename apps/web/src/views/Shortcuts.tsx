import type { Intent } from "../store/msg";
import { SHORTCUTS } from "../store/shortcuts";
import type { State } from "../store/state";
import { Modal } from "./Modal";

/// The list of keyboard shortcuts, opened by "?" or from the sidebar. Escape or "?" closes it.
export function Shortcuts({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  if (!state.shortcutsOpen) return null;
  return (
    <Modal label="Keyboard shortcuts" testId="shortcuts" onClose={() => dispatch({ type: "closeShortcuts" })}>
      <div className="card-block shortcuts">
        <h2>Keyboard shortcuts</h2>
        <dl className="shortcut-list">
          {SHORTCUTS.map((s) => (
            <div key={s.what} className="shortcut" data-testid="shortcut">
              <dt>{s.keys.map((k, i) => <span key={k}>{i > 0 && " + "}<kbd>{k}</kbd></span>)}</dt>
              <dd>{s.what}</dd>
            </div>
          ))}
        </dl>
        <p className="muted">The arrow keys move between the tabs of a story, and between the entries of a menu.</p>
      </div>
    </Modal>
  );
}
