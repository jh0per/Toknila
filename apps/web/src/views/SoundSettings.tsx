import type { SoundSettings as Switches } from "../api/types";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import type { SoundKind } from "../sound/notes";

const ROWS: { kind: SoundKind; label: string }[] = [
  { kind: "story_done", label: "A story is done" },
  { kind: "epic_done", label: "An epic is finished" },
  { kind: "problem", label: "Something went wrong" },
];

/// Settings → Machine → Sounds: a switch for each sound, off until turned on, and a Play button
/// that plays it whether or not its switch is on, so a person chooses by ear.
export function SoundsView({ switches, dispatch }: { switches: Switches; dispatch: (i: Intent) => void }) {
  return (
    <section className="card-block" data-testid="sounds">
      <h2>Sounds</h2>
      <p className="muted">The window plays a short sound when one of these happens while it is open. Your computer's volume sets how loud.</p>
      {ROWS.map(({ kind, label }) => (
        <div className="slots" key={kind}>
          <label>
            <input
              type="checkbox"
              data-testid={`sound-${kind}`}
              checked={switches[kind]}
              onChange={(e) => dispatch({ type: "setSounds", sounds: { ...switches, [kind]: e.target.checked } })}
            />{" "}
            {label}
          </label>
          <button type="button" data-testid={`play-${kind}`} aria-label={`Play the sound for: ${label}`} onClick={() => dispatch({ type: "playPreview", kind })}>
            Play
          </button>
        </div>
      ))}
    </section>
  );
}

export function SoundSettings({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  return <SoundsView switches={state.sound.switches} dispatch={dispatch} />;
}
