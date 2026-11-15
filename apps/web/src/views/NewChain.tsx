import { useState } from "react";
import type { ReviewMode } from "../api/types";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import type { State } from "../store/state";
import { Field } from "./Field";
import { chainInput, slugLine } from "./newChainForm";

const MODES: ReviewMode[] = ["human", "agent", "both"];
const REVIEW_WORDS: Record<ReviewMode, string> = {
  human: "I review every finished story", agent: "A reviewer agent reviews it", both: "A reviewer agent first, then me",
};

/// A new, empty epic in a project: its title (the slug is derived and shown, editable on request),
/// an optional goal, whether ready stories start by themselves, and who reviews. The daemon judges
/// the input; its refusal shows here. `initialTitle` only seeds the form (tests).
export function NewChain({ project, state, dispatch, initialTitle = "" }: { project: string; state: State; dispatch: (i: Intent) => void; initialTitle?: string }) {
  const [title, setTitle] = useState(initialTitle);
  // The slug as the person typed it; none while it follows the title.
  const [typedSlug, setTypedSlug] = useState<string | undefined>(undefined);
  const [editing, setEditing] = useState(false);
  const [empty, setEmpty] = useState(false);
  const { slug } = slugLine({ title, typed: typedSlug });
  const [goal, setGoal] = useState("");
  // The version starts as the one the project suggests; what the person types wins.
  const [typed, setTyped] = useState<string | undefined>(undefined);
  const version = typed ?? state.epic.suggested ?? "";
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
        if (!title.trim()) {
          setEmpty(true);
          return;
        }
        dispatch({ type: "createChain", project, input: chainInput({ title, slug, goal, autostart, mode, version }) });
      }}
    >
      <h2>New epic in {project}</h2>
      <Field
        label="Title" name="chain-title"
        error={empty && !title.trim() ? "An epic needs a title." : planError(p, "chain", "title")}
      >
        <input data-testid="chain-title" value={title} placeholder="MIDI controllers" autoFocus onChange={(e) => { setTitle(e.target.value); setEmpty(false); }} />
      </Field>
      {editing ? (
        <Field label="Slug" name="chain-name" error={planError(p, "chain", "name")} hint="Folders and branches use it. Lower-case letters, digits and hyphens.">
          <input data-testid="chain-name" value={slug ?? ""} spellCheck={false} onChange={(e) => setTypedSlug(e.target.value)} />
        </Field>
      ) : (
        <p className="field-hint" data-testid="chain-slug">
          {slug ? <>becomes <code data-testid="chain-slug-value">{slug}</code>; folders and branches use it</> : "the slug is made from the title"}
          {" "}
          <button type="button" className="link" data-testid="chain-slug-change" onClick={() => { setTypedSlug(slug ?? ""); setEditing(true); }}>Change</button>
          {planError(p, "chain", "name") && <span className="field-error" role="alert" data-testid="error-chain-name"> {planError(p, "chain", "name")}</span>}
        </p>
      )}
      <Field label="Goal (optional)" name="chain-goal" error={undefined}>
        <textarea data-testid="chain-goal" rows={2} value={goal} onChange={(e) => setGoal(e.target.value)} />
      </Field>
      <Field
        label="Version (optional)" name="chain-version" error={planError(p, "chain", "version")}
        hint={state.epic.suggested ? `The epic ends in a release of this version; suggested: the next patch after the newest tag and the versions the roadmap plans.` : "The epic ends in a release of this version."}
      >
        <input data-testid="chain-version" value={version} placeholder="0.3.0" spellCheck={false} onChange={(e) => setTyped(e.target.value)} />
      </Field>
      <label className="check">
        <input type="checkbox" data-testid="chain-autostart" checked={autostart} onChange={(e) => setAutostart(e.target.checked)} />
        Start stories automatically
      </label>
      <Field label="Who reviews finished stories" name="chain-review-mode" error={undefined}>
        <select data-testid="chain-review-mode" value={mode} onChange={(e) => setMode(e.target.value as ReviewMode | "")}>
          <option value="">The project's setting</option>
          {MODES.map((m) => <option key={m} value={m}>{REVIEW_WORDS[m]}</option>)}
        </select>
      </Field>
      {refused && <p className="error" role="alert" data-testid="chain-error">{refused}</p>}
      <button className="btn primary" type="submit" disabled={p.busy} data-testid="create-chain">Create epic</button>
    </form>
  );
}
