import { useEffect, useRef, useState } from "react";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import type { QuickCard as Quick } from "../store/quick";
import type { State } from "../store/state";

/// The small new-card field on the canvas: a title, Enter creates the card, Escape or Cancel
/// closes it. The daemon's refusal shows under the field as it said it.
export function QuickCard({ quick, label, state, dispatch }: {
  quick: Quick;
  /// What the field says it makes, e.g. "New card after EDG-1".
  label: string;
  state: State;
  dispatch: (i: Intent) => void;
}) {
  const [title, setTitle] = useState("");
  const ref = useRef<HTMLFormElement>(null);
  const close = () => dispatch({ type: "closeQuickCard" });
  useEffect(() => {
    const down = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) dispatch({ type: "closeQuickCard" });
    };
    window.addEventListener("mousedown", down);
    return () => window.removeEventListener("mousedown", down);
  }, [dispatch]);
  const refused = planError(state.plan, "card");
  return (
    <form
      ref={ref}
      className="quick-card"
      data-testid="quick-card"
      style={{ left: quick.x, top: quick.y }}
      onClick={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
      onContextMenu={(e) => e.stopPropagation()}
      onSubmit={(e) => {
        e.preventDefault();
        dispatch({ type: "submitQuickCard", title });
      }}
    >
      <label>
        <span data-testid="quick-card-label">{label}</span>
        <input
          data-testid="quick-card-title" value={title} autoFocus placeholder="Title, then Enter"
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.stopPropagation();
              close();
            }
          }}
        />
      </label>
      {refused && <p className="error" role="alert" data-testid="quick-card-error">{refused}</p>}
      <button type="button" className="link-btn" data-testid="quick-card-cancel" onClick={close}>Cancel</button>
    </form>
  );
}
