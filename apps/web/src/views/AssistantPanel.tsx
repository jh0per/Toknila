import { type KeyboardEvent, type PointerEvent, useEffect, useState } from "react";
import type { Project } from "../api/types";
import { MAX_WIDTH, MIN_WIDTH, OVERLAY_PX, chipOf } from "../store/assistant";
import type { Intent } from "../store/msg";
import { plannerOf } from "../store/planner";
import type { State } from "../store/state";
import { ChatPane } from "./ChatPane";

const KEY_STEP_PX = 24;

/// Whether the window is narrower than the overlay breakpoint, followed as it resizes.
export function useOverlay(): boolean {
  const query = () => window.matchMedia(`(max-width: ${OVERLAY_PX - 1}px)`);
  const [overlay, setOverlay] = useState(() => query().matches);
  useEffect(() => {
    const m = query();
    const on = () => setOverlay(m.matches);
    m.addEventListener("change", on);
    return () => m.removeEventListener("change", on);
  }, []);
  return overlay;
}

/// The handle on the panel's left edge: drag it, or use the arrow keys, to change the width. The
/// width is the state's (and so remembered for this viewer).
function WidthHandle({ width, dispatch }: { width: number; dispatch: (i: Intent) => void }) {
  const [grab, setGrab] = useState(false);
  const set = (px: number) => dispatch({ type: "setAssistantWidth", width: px });
  const move = (e: PointerEvent) => grab && set(window.innerWidth - e.clientX);
  const key = (e: KeyboardEvent) => {
    const step = e.key === "ArrowLeft" ? KEY_STEP_PX : e.key === "ArrowRight" ? -KEY_STEP_PX : 0;
    if (!step) return;
    e.preventDefault();
    set(width + step);
  };
  return (
    <div
      className="assistant-handle" role="separator" aria-orientation="vertical" tabIndex={0} aria-label="Resize the assistant panel"
      aria-valuenow={width} aria-valuemin={MIN_WIDTH} aria-valuemax={MAX_WIDTH} data-testid="assistant-handle" title="Drag to resize the panel"
      onPointerDown={(e) => { if (e.button === 0) { e.currentTarget.setPointerCapture(e.pointerId); setGrab(true); e.preventDefault(); } }}
      onPointerMove={move} onPointerUp={() => setGrab(false)} onPointerCancel={() => setGrab(false)} onKeyDown={key}
    />
  );
}

/// The assistant beside the work: the project's one chat, on the right of the roadmap, a chain or
/// a card, with a chip that says what is open. Under 1100 px it overlays the screen with a scrim and
/// Escape closes it.
export function AssistantPanel({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const overlay = useOverlay();
  const running = plannerOf(state, project.key)?.running;
  useEffect(() => {
    if (!overlay) return;
    const esc = (e: globalThis.KeyboardEvent) => e.key === "Escape" && dispatch({ type: "closeAssistant" });
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [overlay, dispatch]);
  return (
    <>
      {overlay && <div className="assistant-scrim" data-testid="assistant-scrim" onClick={() => dispatch({ type: "closeAssistant" })} />}
      <aside className={`assistant-panel${overlay ? " overlay" : ""}`} data-testid="assistant-panel" aria-label="Assistant">
        {!overlay && <WidthHandle width={state.assistant.width} dispatch={dispatch} />}
        <header className="assistant-head">
          <b>Assistant</b>
          <span className="muted" data-testid="assistant-state">{running === undefined ? "…" : running ? "running" : "not running"}</span>
          <span className="grow" />
          <button type="button" className="icon-btn" data-testid="assistant-close" aria-label="Close the assistant panel" onClick={() => dispatch({ type: "closeAssistant" })}>
            ×
          </button>
        </header>
        {state.planner.error && <p className="error side-note" role="alert" data-testid="assistant-error">{state.planner.error}</p>}
        <ChatPane key={project.key} project={project} state={state} dispatch={dispatch} context={chipOf(state)} />
      </aside>
    </>
  );
}
