import type { KeyboardEvent } from "react";
import type { Intent } from "../store/msg";
import { canSend } from "../store/askAttach";
import { draftOf } from "../store/planner";
import type { State } from "../store/state";
import { AskChips, AttachButton, DropFrame, useAskAttach } from "./AskAttach";

/// A text box that sends on Enter (Shift+Enter makes a new line). It does not send while an input
/// method is composing a character.
export function PromptField({ value, onChange, onSubmit, placeholder, rows = 3, disabled = false, label, testId }: {
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
  return (
    <textarea
      className="prompt-field" aria-label={label} data-testid={testId} rows={rows} value={value} disabled={disabled}
      placeholder={placeholder} onChange={(e) => onChange(e.target.value)} onKeyDown={key}
    />
  );
}

/// The box that talks to a project's assistant (its planner): what is typed is kept in the store,
/// Enter or the button starts the assistant if it is not running and sends it the text. While the
/// text is on its way the box waits.
export function AskBox({ project, state, dispatch, placeholder, button, label, rows, testId, compact = false, waiting = false }: {
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
  return (
    <DropFrame a={a} className={`ask${compact ? " compact" : ""}`} testId={testId}>
      <PromptField
        value={text} label={label} testId={`${testId}-input`} rows={rows ?? (compact ? 2 : 3)} disabled={sending}
        placeholder={placeholder} onSubmit={send}
        onChange={(t) => dispatch({ type: "editPlannerDraft", where: project, text: t })}
      />
      <AskChips a={a} project={project} testId={testId} />
      <div className="ask-actions">
        <AttachButton busy={sending || a.place.busy} onFiles={a.add} testId={testId} />
        <button
          type="button" className="btn primary" data-testid={`${testId}-send`} onClick={send}
          disabled={sending || !canSend(state, project, text)}
        >
          {state.planner.sending === project ? "Sending…" : button}
        </button>
      </div>
    </DropFrame>
  );
}
