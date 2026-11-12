import { useState } from "react";
import type { ChainDetail } from "../api/types";
import type { EpicKind } from "../store/epic";
import type { Intent } from "../store/msg";
import { dayText, finishView } from "../store/phase";
import type { State } from "../store/state";
import { Field } from "./Field";
import { Markdown } from "./Markdown";

/// The side panel of the Start node (the epic: name and goal) or of the Finish node (the release:
/// its version, editable while nothing is released). The daemon judges the input; its refusal shows
/// under the form.
export function EpicPanel({ chain, kind, project, state, dispatch }: {
  chain: ChainDetail;
  kind: EpicKind;
  project: string;
  state: State;
  dispatch: (i: Intent) => void;
}) {
  const head = kind === "epic" ? "Epic" : "Release";
  return (
    <section className="panel epic-panel" data-testid="epic-panel" data-kind={kind}>
      <div className="panel-head">
        <div className="tabs"><span className="who"><span className="key">{head}</span> {chain.chain.name}</span></div>
        <button type="button" className="btn small" data-testid="epic-close" onClick={() => dispatch({ type: "closeEpic" })}>Close</button>
      </div>
      <div className="tabpanel">
        {kind === "epic"
          ? <EpicForm key={chain.chain.id} chain={chain} project={project} state={state} dispatch={dispatch} />
          : <ReleaseForm key={chain.chain.id} chain={chain} project={project} state={state} dispatch={dispatch} />}
      </div>
    </section>
  );
}

type FormProps = { chain: ChainDetail; project: string; state: State; dispatch: (i: Intent) => void };

function EpicForm({ chain, project, state, dispatch }: FormProps) {
  const [name, setName] = useState(chain.chain.name);
  const [goal, setGoal] = useState(chain.chain.goal);
  const [preview, setPreview] = useState(false);
  const changed = name.trim() !== chain.chain.name || goal.trim() !== chain.chain.goal.trim();
  const save = () => {
    const patch: { name?: string; goal?: string } = {};
    if (name.trim() !== chain.chain.name) patch.name = name.trim();
    if (goal.trim() !== chain.chain.goal.trim()) patch.goal = goal;
    dispatch({ type: "saveEpic", chain: chain.chain.id, project, from: chain.chain.name, patch });
  };
  return (
    <form className="form epic-form" noValidate onSubmit={(e) => { e.preventDefault(); if (changed) save(); }}>
      <Field label="Name" name="epic-name" error={undefined}>
        <input data-testid="epic-name" value={name} spellCheck={false} onChange={(e) => setName(e.target.value)} />
      </Field>
      <Field label="Goal" name="epic-goal" error={undefined} hint="What the epic is for. The planner and every story's agent read it as the epic's purpose. Markdown.">
        {preview
          ? <div className="epic-preview" data-testid="epic-preview"><Markdown text={goal} /></div>
          : <textarea data-testid="epic-goal-input" rows={6} value={goal} onChange={(e) => setGoal(e.target.value)} />}
      </Field>
      <div className="row-actions">
        <button type="button" className="btn small" data-testid="epic-preview-toggle" onClick={() => setPreview(!preview)}>{preview ? "Edit" : "Preview"}</button>
        <button type="submit" className="btn primary" data-testid="epic-save" disabled={!changed || state.epic.busy}>Save</button>
      </div>
      {state.epic.error && <p className="error" role="alert" data-testid="epic-error">{state.epic.error}</p>}
    </form>
  );
}

function ReleaseForm({ chain, project, state, dispatch }: FormProps) {
  const view = finishView(chain.summary);
  const own = chain.summary.version ?? "";
  const [version, setVersion] = useState(own);
  // The version is fixed once the release was cut: it is the tag.
  const locked = chain.summary.finish?.state === "released" || chain.releases.some((r) => r.state === "cut");
  const changed = version.trim() !== own;
  const f = chain.summary.finish;
  return (
    <form
      className="form epic-form" noValidate
      onSubmit={(e) => {
        e.preventDefault();
        if (changed && !locked) dispatch({ type: "saveEpic", chain: chain.chain.id, project, from: chain.chain.name, patch: { version: version.trim() } });
      }}
    >
      <p data-testid="release-panel-state">
        {view ? <><b>{view.title}</b> · {view.stateText}{f?.released_at != null && ` on ${dayText(f.released_at)}`}{f?.tag && ` · tag ${f.tag}`}</> : "This epic has no version yet."}
      </p>
      <Field
        label="Version" name="epic-version" error={undefined}
        hint={locked ? "The release was cut: its version is its tag." : "Editable while nothing is released. It becomes the tag, such as v0.3.0."}
      >
        <input data-testid="epic-version" value={version} disabled={locked} spellCheck={false} placeholder="0.3.0" onChange={(e) => setVersion(e.target.value)} />
      </Field>
      <div className="row-actions">
        <button type="submit" className="btn primary" data-testid="epic-save" disabled={!changed || locked || state.epic.busy}>Save</button>
        {chain.summary.releasable && (
          <button type="button" className="btn" data-testid="panel-release" onClick={() => dispatch({ type: "openRelease", chain: chain.chain.id })}>
            {f?.state === "again" ? "Release again…" : chain.releases.some((r) => r.state === "cut") ? "Continue release…" : "Release…"}
          </button>
        )}
      </div>
      {state.epic.error && <p className="error" role="alert" data-testid="epic-error">{state.epic.error}</p>}
    </form>
  );
}
