import { fieldError } from "../store/settings";
import { Field } from "./Field";
import { ModelPicker } from "./ModelPicker";
import type { FormProps } from "./ProjectSections";

type ModelName = "planner_model" | "worker_model" | "reviewer_model";

const MODELS: { name: ModelName; label: string; testId: string; hint: string }[] = [
  { name: "planner_model", label: "Assistant", testId: "planner-model", hint: "Plans the work and talks with you." },
  { name: "worker_model", label: "Card agents", testId: "worker-model", hint: "Does the work of a card, unless the card names its own model." },
  { name: "reviewer_model", label: "Reviewer", testId: "reviewer-model", hint: "Reads what a card agent handed in." },
];

/// The Claude models of the project, named for the person who does the work: the assistant (the
/// planner), the card agents (the workers) and the reviewer.
export function ModelsFields({ state, form, set }: Pick<FormProps, "state" | "form" | "set">) {
  return (
    <div data-testid="project-models">
      <h3 className="sub">Models</h3>
      {MODELS.map((m) => (
        <Field key={m.name} label={m.label} name={m.name} error={fieldError(state.settings, m.name)} hint={m.hint}>
          <ModelPicker value={form[m.name]} onChange={(v) => set(m.name, v)} testId={m.testId} defaultLabel="Claude Code's default" />
        </Field>
      ))}
      <p className="muted" data-testid="model-note">A change applies to the next agent that starts.</p>
    </div>
  );
}
