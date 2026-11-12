import type { Speech } from "./useSpeech";

/// The microphone button of a composer: press to listen, press again to stop. It is not drawn when
/// the window has no recognizer (the hint says what to do instead).
export function MicButton({ s, testId, disabled }: { s: Speech; testId: string; disabled: boolean }) {
  if (s.mic === "none") return null;
  const on = s.mic === "listening";
  const label = on ? "Stop listening" : "Speak to the assistant";
  return (
    <button
      type="button" className={`btn icon mic-btn mic-${s.mic}`} data-testid={`${testId}-mic`} data-mic={s.mic}
      aria-label={label} aria-pressed={on} title={label} disabled={disabled} onClick={s.toggle}
    >
      <span aria-hidden="true">{s.mic === "denied" ? "🚫" : "🎤"}</span>
    </button>
  );
}

/// Under the field while the microphone is in play: the words not yet settled (muted), the
/// sentence for a refusal or a failure, and the setting "send when I stop speaking".
export function SpeechNote({ s, testId }: { s: Speech; testId: string }) {
  const failed = s.mic === "denied" || s.mic === "error";
  return (
    <>
      {s.mic === "listening" && (
        <p className="mic-live" role="status" data-testid={`${testId}-mic-live`}>
          {s.interim ? <span className="mic-interim" data-testid={`${testId}-mic-interim`}>{s.interim}</span> : "Listening… speak, then press the button"}
        </p>
      )}
      {s.message && (
        <p className={`mic-note${failed ? " error" : ""}`} role={failed ? "alert" : undefined} data-testid={`${testId}-mic-note`}>{s.message}</p>
      )}
      {s.mic !== "none" && (
        <label className="mic-auto">
          <input type="checkbox" checked={s.auto} data-testid={`${testId}-mic-auto`} onChange={(e) => s.setAuto(e.target.checked)} />
          Send when I stop speaking
        </label>
      )}
    </>
  );
}
