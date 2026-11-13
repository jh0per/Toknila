import type { AgentRole } from "../api/types";
import type { Intent } from "../store/msg";

/// The intent of pressing the choice at `index` (0-based): the answer route tells the agent the
/// choice's own text.
export function choiceIntent(key: string, role: AgentRole, index: number): Intent {
  return { type: "answer", key, role, answer: "text", choice: index };
}

/// The things the agent offered to do about its question, one button each, the recommended one
/// first. Pressing a button answers at once.
export function Choices({ choices, onPick }: { choices: string[]; onPick: (index: number) => void }) {
  return (
    <div className="choices" data-testid="choices">
      {choices.map((choice, i) => (
        <button
          key={i}
          type="button"
          className={i === 0 ? "btn primary" : "btn"}
          data-testid={`choice-${i}`}
          data-recommended={i === 0}
          onClick={() => onPick(i)}
        >
          {choice}
          {i === 0 && <span className="muted"> (recommended)</span>}
        </button>
      ))}
    </div>
  );
}
