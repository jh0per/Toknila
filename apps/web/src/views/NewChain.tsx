import { useState } from "react";
import type { ReviewMode } from "../api/types";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import type { State } from "../store/state";
import { Field } from "./Field";

const MODES: ReviewMode[] = ["human", "agent", "both"];
const REVIEW_WORDS: Record<ReviewMode, string> = {
  human: "I review every finished card", agent: "A reviewer agent reviews it", both: "A reviewer agent first, then me",
};

/// A new, empty chain in a project: its name, an optional goal, whether ready cards start by
/// themselves, and who reviews. The daemon judges the input; its refusal shows here.
export function NewChain({ project, state, dispatch }: { project: string; state: State; dispatch: (i: Intent) => void }) {
  const [name, setName] = useState("");
  const [goal, setGoal] = useState("");
  const [autostart, setAutostart] = useState(true);
  const [mode, setMode] = useState<ReviewMode | "">("");
  const p = state.plan;
  const refused = planError(p, "chain");
  return (
    <form
      className="card-block form"
      data-testid="new-chain"
      noValidate
      onSubmit={(e) => {
        e.preventDefault();
        dispatch({
          type: "createChain",
          project,
          input: { name: name.trim(), goal: goal.trim() || null, autostart, review_mode: mode || null },
        });
      }}
    >
      <h2>New chain in {project}</h2>
      <Field label="Name" name="chain-name" error={planError(p, "chain", "name")}>
        <input data-testid="chain-name" value={name} placeholder="window-planning" spellCheck={false} autoFocus onChange={(e) => setName(e.target.value)} />
      </Field>
      <Field label="Goal (optional)" name="chain-goal" error={undefined}>
        <textarea data-testid="chain-goal" rows={2} value={goal} onChange={(e) => setGoal(e.target.value)} />
      </Field>
      <label className="check">
        <input type="checkbox" data-testid="chain-autostart" checked={autostart} onChange={(e) => setAutostart(e.target.checked)} />
        Start cards automatically
      </label>
      <Field label="Who reviews finished cards" name="chain-review-mode" error={undefined}>
        <select data-testid="chain-review-mode" value={mode} onChange={(e) => setMode(e.target.value as ReviewMode | "")}>
          <option value="">The project's setting</option>
          {MODES.map((m) => <option key={m} value={m}>{REVIEW_WORDS[m]}</option>)}
        </select>
      </Field>
      {refused && <p className="error" role="alert" data-testid="chain-error">{refused}</p>}
      <button className="btn primary" type="submit" disabled={p.busy} data-testid="create-chain">Create chain</button>
    </form>
  );
}
