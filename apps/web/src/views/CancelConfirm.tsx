import { useEffect, useRef, useState } from "react";
import { CANCEL_REASON } from "../store/quick";

/// The question before a card is cancelled, shared by the card bar and the diagram's menu: the
/// reason (the daemon asks for one) is prefilled and editable; Enter or the button sends it.
/// Escape, "Keep it" or a click elsewhere drops the question.
export function CancelConfirm({ cardKey, onSend, onKeep }: {
  cardKey: string;
  onSend: (reason: string) => void;
  onKeep: () => void;
}) {
  const [reason, setReason] = useState(CANCEL_REASON);
  const ref = useRef<HTMLFormElement>(null);
  useEffect(() => {
    const down = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onKeep();
    };
    window.addEventListener("mousedown", down);
    return () => window.removeEventListener("mousedown", down);
  }, [onKeep]);
  return (
    <form
      ref={ref}
      className="cancel-confirm"
      data-testid="cancel-confirm"
      onClick={(e) => e.stopPropagation()}
      onSubmit={(e) => {
        e.preventDefault();
        onSend(reason.trim());
      }}
    >
      <label>
        Cancel {cardKey}? Reason
        <input
          data-testid="cancel-reason" value={reason} autoFocus onChange={(e) => setReason(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.stopPropagation();
              onKeep();
            }
          }}
        />
      </label>
      <span className="btns">
        <button type="submit" className="danger" data-testid="cancel-send">Cancel card</button>
        <button type="button" data-testid="cancel-keep" onClick={onKeep}>Keep it</button>
      </span>
    </form>
  );
}
