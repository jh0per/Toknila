import { useState } from "react";
import type { Ticket } from "../api/types";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import type { State } from "../store/state";
import { ModelPicker } from "./ModelPicker";

/// The model a card's agent runs on: the card's own, else the project's. While the card can still
/// be edited a small editor changes it; "default" hands the choice back to the project.
export function CardModel({ state, card, canEdit, projectModel, dispatch }: {
  state: State;
  card: Ticket;
  canEdit: boolean;
  projectModel: string | undefined;
  dispatch: (i: Intent) => void;
}) {
  const own = card.model ?? "";
  const [draft, setDraft] = useState(own);
  const shown = own || projectModel || "Claude Code's default";
  if (!canEdit) {
    return (
      <p className="muted" data-testid="card-model">
        Model: <code>{shown}</code>{own ? "" : " (the project's)"}
      </p>
    );
  }
  const refused = planError(state.plan, "edit");
  return (
    <form
      className="plan-form" data-testid="card-model-editor"
      onSubmit={(e) => {
        e.preventDefault();
        dispatch({ type: "saveCard", key: card.key, patch: { model: draft.trim() } });
      }}
    >
      <label className="field">
        <span className="field-label">Model</span>
        <span className="field-hint">Applies when the agent starts. By default the project's model: {projectModel ?? "Claude Code's default"}.</span>
        <ModelPicker value={draft} onChange={setDraft} testId="card-model-select" defaultLabel="The project's model" />
      </label>
      {refused && <p className="error" role="alert" data-testid="card-model-error">{refused}</p>}
      {draft.trim() !== own && (
        <button className="btn" type="submit" disabled={state.plan.busy} data-testid="save-card-model">Save model</button>
      )}
    </form>
  );
}
