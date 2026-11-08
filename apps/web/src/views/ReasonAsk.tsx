import { useEffect, useRef, useState } from "react";

const WORDS = {
  reject: { ask: "Why is {key} turned down?", send: "Reject card" },
  return: { ask: "What should the agent change in {key}?", send: "Return card" },
} as const;

/// The question before a card is rejected or sent back from the diagram: the daemon wants the
/// person's words with either, so the field is empty and the button waits for them. Enter or the
/// button sends; Escape, "Keep it" or a click elsewhere drops the question.
export function ReasonAsk({ cardKey, action, onSend, onKeep }: {
  cardKey: string;
  action: keyof typeof WORDS;
  onSend: (text: string) => void;
  onKeep: () => void;
}) {
  const [text, setText] = useState("");
  const ref = useRef<HTMLFormElement>(null);
  useEffect(() => {
    const down = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onKeep();
    };
    window.addEventListener("mousedown", down);
    return () => window.removeEventListener("mousedown", down);
  }, [onKeep]);
  const words = WORDS[action];
  return (
    <form
      ref={ref} className="cancel-confirm" data-testid="reason-ask" data-action={action}
      onClick={(e) => e.stopPropagation()}
      onSubmit={(e) => {
        e.preventDefault();
        if (text.trim()) onSend(text.trim());
      }}
    >
      <label>
        {words.ask.replace("{key}", cardKey)}
        <input
          data-testid="reason-text" value={text} autoFocus onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.stopPropagation();
              onKeep();
            }
          }}
        />
      </label>
      <span className="btns">
        <button type="submit" className={action === "reject" ? "danger" : ""} data-testid="reason-send" disabled={!text.trim()}>
          {words.send}
        </button>
        <button type="button" data-testid="reason-keep" onClick={onKeep}>Keep it</button>
      </span>
    </form>
  );
}
