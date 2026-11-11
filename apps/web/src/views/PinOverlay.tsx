import { useState } from "react";
import { regionStyle } from "../store/designs";
import { MAX_ANSWER, type Pin, answerProblem, isOpen, pinPlace } from "../store/pins";

/// Numbered markers of the assistant's pins, drawn on a picture's stage in percent of the picture
/// (so they stay put at every zoom). An open pin is filled, an answered one is a ring with a
/// check. The selected pin also outlines its region.
export function PinMarkers({ pins, active, onSelect }: { pins: Pin[]; active: string | undefined; onSelect: (id: string) => void }) {
  return (
    <>
      {pins.map((p) => (
        <span key={p.id}>
          {p.id === active && <span className="pin-region" data-testid="pin-region" style={regionStyle(p.region)} />}
          <button
            type="button" className="pin-marker" data-testid="pin-marker" data-number={p.number}
            data-state={isOpen(p) ? "open" : "answered"} aria-pressed={p.id === active}
            aria-label={`Pin ${p.number}, ${isOpen(p) ? "open" : "answered"}: ${p.text}`}
            style={{ left: `${p.region.x}%`, top: `${p.region.y}%` }} onClick={() => onSelect(p.id)}
          >
            {isOpen(p) ? p.number : `${p.number}✓`}
          </button>
        </span>
      ))}
    </>
  );
}

/// The pins as questions of the assistant under a picture: the question with its place, and for an
/// open pin the box that answers it in place; an answered pin shows the answer and can change it.
export function PinList({ pins, active, onSelect, onAnswer }: {
  pins: Pin[];
  active: string | undefined;
  onSelect: (id: string) => void;
  onAnswer: (id: string, text: string) => Promise<void>;
}) {
  if (pins.length === 0) return null;
  return (
    <ul className="pin-list" data-testid="pin-list">
      {pins.map((p) => (
        <li
          key={p.id} className="pin-item" data-testid="pin" data-number={p.number}
          data-state={isOpen(p) ? "open" : "answered"} data-active={p.id === active}
        >
          <button type="button" className="pin-head" onClick={() => onSelect(p.id)}>
            <span className="pin-number">{p.number}</span>
            <span className="muted">{isOpen(p) ? "The assistant asks" : "The assistant asked"} · {pinPlace(p)}</span>
          </button>
          <p className="pin-text" data-testid="pin-text">{p.text}</p>
          <PinAnswer pin={p} onAnswer={onAnswer} />
        </li>
      ))}
    </ul>
  );
}

/// The answer of one pin: a box while it is open (or being changed), the answer otherwise.
function PinAnswer({ pin, onAnswer }: { pin: Pin; onAnswer: (id: string, text: string) => Promise<void> }) {
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(pin.answer ?? "");
  const [error, setError] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);
  if (!isOpen(pin) && !editing) {
    return (
      <p className="pin-answer" data-testid="pin-answer">
        <span className="muted">You: </span>{pin.answer}{" "}
        <button type="button" className="btn small" data-testid="pin-edit" onClick={() => { setText(pin.answer ?? ""); setEditing(true); }}>Edit</button>
      </p>
    );
  }
  const send = async () => {
    const problem = answerProblem(text);
    if (problem) return setError(problem);
    setBusy(true);
    setError(undefined);
    try {
      await onAnswer(pin.id, text.trim());
      setEditing(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <form className="pin-form" onSubmit={(e) => { e.preventDefault(); void send(); }}>
      <input
        type="text" data-testid="pin-input" aria-label={`Answer to pin ${pin.number}`} placeholder="Your answer"
        maxLength={MAX_ANSWER} value={text} onChange={(e) => setText(e.target.value)}
      />
      <button type="submit" className="btn primary" data-testid="pin-send" disabled={busy || !text.trim()}>Send</button>
      {error && <span className="field-error" role="alert" data-testid="pin-error">{error}</span>}
    </form>
  );
}
