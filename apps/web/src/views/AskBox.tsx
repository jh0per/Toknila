import { type KeyboardEvent, useEffect, useLayoutEffect, useRef } from "react";
import type { Intent } from "../store/msg";
import { canSend } from "../store/askAttach";
import { draftOf } from "../store/planner";
import type { State } from "../store/state";
import { AskChips, AttachButton, DropFrame, useAskAttach } from "./AskAttach";
import { MicButton, SpeechNote } from "./MicButton";
import { useSpeech } from "./useSpeech";

/// The most rows the chat's composer grows to.
const MAX_ROWS = 8;

/// A text box that sends on Enter (Shift+Enter makes a new line). It does not send while an input
/// method is composing a character.
export function PromptField({ value, onChange, onSubmit, placeholder, rows = 3, grow, disabled = false, label, testId, focus = false }: {
  /// Grow with the text from `rows` rows up to this many rows, then scroll.
  grow?: number;
  /// Take the focus when it appears.
  focus?: boolean;
  value: string;
  onChange: (text: string) => void;
  onSubmit: () => void;
  placeholder: string;
  rows?: number;
  disabled?: boolean;
  label: string;
  testId: string;
}) {
  const key = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key !== "Enter" || e.shiftKey || e.nativeEvent.isComposing) return;
    e.preventDefault();
    onSubmit();
  };
  const box = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (focus && !disabled) box.current?.focus();
  }, [focus, disabled]);
  useLayoutEffect(() => {
    const el = box.current;
    if (!el || !grow) return;
    const cs = getComputedStyle(el);
    const line = parseFloat(cs.lineHeight);
    if (!Number.isFinite(line)) return;
    const frame = parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom) + parseFloat(cs.borderTopWidth) + parseFloat(cs.borderBottomWidth);
    el.style.height = "auto";
    const max = line * grow + frame;
    el.style.height = `${Math.min(el.scrollHeight + parseFloat(cs.borderTopWidth) + parseFloat(cs.borderBottomWidth), max)}px`;
    el.style.overflowY = el.scrollHeight > max ? "auto" : "hidden";
  }, [value, grow]);
  return (
    <textarea
      ref={box} className="prompt-field" aria-label={label} data-testid={testId} rows={rows} value={value} disabled={disabled}
      placeholder={placeholder} onChange={(e) => onChange(e.target.value)} onKeyDown={key}
    />
  );
}

/// The box that talks to a project's assistant (its planner): what is typed is kept in the store,
/// Enter or the button starts the assistant if it is not running and sends it the text. While the
/// text is on its way the box waits.
export function AskBox({ project, state, dispatch, placeholder, button, label, rows, testId, compact = false, waiting = false, focus = false }: {
  /// The box takes the focus when it appears (a starter points at it).
  focus?: boolean;
  project: string;
  state: State;
  dispatch: (i: Intent) => void;
  placeholder: string;
  button: string;
  label: string;
  rows?: number;
  testId: string;
  /// A single row beside its button, under a terminal.
  compact?: boolean;
  /// The assistant is answering the last message: the box waits, as it does while sending.
  waiting?: boolean;
}) {
  const text = draftOf(state, project);
  const sending = state.planner.sending === project || waiting;
  const send = () => dispatch({ type: "askPlanner", project });
  const a = useAskAttach(state, dispatch, { project, where: project, prefix: project });
  const setText = (t: string) => dispatch({ type: "editPlannerDraft", where: project, text: t });
  const speech = useSpeech(text, setText, send);
  return (
    <DropFrame a={a} className={`ask${compact ? " compact" : ""}`} testId={testId}>
      <PromptField
        value={text} label={label} testId={`${testId}-input`} rows={rows ?? 3} grow={compact ? MAX_ROWS : undefined} disabled={sending} focus={focus}
        placeholder={placeholder} onSubmit={send}
        onChange={(t) => { speech.stop(); setText(t); }}
      />
      <AskChips a={a} project={project} testId={testId} />
      <div className="ask-actions">
        <AttachButton busy={sending || a.place.busy} onFiles={a.add} testId={testId} />
        <div className="ask-actions-end">
          <MicButton s={speech} testId={testId} disabled={sending} />
          <button
            type="button" className={`btn send ${compact ? "tonal" : "primary"}`} data-testid={`${testId}-send`} onClick={send}
            disabled={sending || !canSend(state, project, text)}
          >
            {state.planner.sending === project ? "Sending…" : button}
          </button>
        </div>
      </div>
      <SpeechNote s={speech} testId={testId} />
      <p className="ask-hint">Enter sends, Shift+Enter for a new line</p>
    </DropFrame>
  );
}
