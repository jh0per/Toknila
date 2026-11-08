import type { Intent } from "../store/msg";
import { MODEL_ALIASES } from "../store/models";
import { fieldError } from "../store/settings";
import type { State } from "../store/state";
import { ArchiveProject } from "./ArchiveProject";
import { Field } from "./Field";
import type { FormProps } from "./ProjectSections";

/// One model field: free text with the documented aliases offered.
function ModelField({ label, name, testId, form, set, state }: FormProps & {
  label: string; name: "worker_model" | "reviewer_model" | "planner_model"; testId: string;
}) {
  return (
    <Field label={label} name={name} error={fieldError(state.settings, name)}>
      <input data-testid={testId} value={form[name]} spellCheck={false} list="model-aliases" onChange={(e) => set(name, e.target.value)} />
    </Field>
  );
}

/// Where Toknila keeps its files on this machine.
function Places({ state }: { state: State }) {
  const places = state.settings.places;
  return (
    <details className="fold" data-testid="places">
      <summary>Where things live</summary>
      <dl className="facts">
        <dt>Data</dt>
        <dd data-testid="place-home"><code>{places?.home ?? "…"}</code></dd>
        <dt>Workspaces</dt>
        <dd data-testid="place-ships"><code>{places?.ships ?? "…"}</code></dd>
        <dt>Agent files</dt>
        <dd data-testid="place-profiles"><code>{places?.profiles ?? "…"}</code></dd>
      </dl>
      <p className="muted">The data folder holds the database and the run logs; a workspace is the copy of the project a card's agent works in.</p>
    </details>
  );
}

/// Advanced: the Claude models, the release branch when it is not the main branch, where things
/// live, and the danger zone (archive, delete), the last two folded away.
export function AdvancedSection({ project, state, form, set, dispatch }: FormProps & { dispatch: (i: Intent) => void }) {
  const props = { project, state, form, set };
  return (
    <div className="page-section" data-testid="project-advanced">
      {project.release_branch !== project.work_branch && (
        <dl className="facts">
          <dt>Release branch</dt>
          <dd data-testid="release-branch">{project.release_branch}</dd>
        </dl>
      )}
      <h3 className="sub">Claude models</h3>
      <ModelField {...props} label="Model of card agents" name="worker_model" testId="worker-model" />
      <ModelField {...props} label="Model of reviewers" name="reviewer_model" testId="reviewer-model" />
      <ModelField {...props} label="Model of the planner" name="planner_model" testId="planner-model" />
      <datalist id="model-aliases">{MODEL_ALIASES.map((m) => <option key={m} value={m} />)}</datalist>
      <p className="muted" data-testid="model-aliases">
        Leave empty to use your Claude Code default. Names you can type: {MODEL_ALIASES.join(", ")}, or a full model name.
      </p>
      <Places state={state} />
      <details className="fold danger-fold" data-testid="danger-zone">
        <summary>Danger zone</summary>
        <ArchiveProject project={project} state={state} dispatch={dispatch} />
      </details>
    </div>
  );
}
