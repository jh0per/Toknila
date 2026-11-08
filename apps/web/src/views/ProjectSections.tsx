import type { MergeMethod, MergeMode, Project, ReviewMode } from "../api/types";
import { type ProjectForm } from "../store/projectForm";
import { listProfile } from "../store/profiles";
import { fieldError } from "../store/settings";
import type { State } from "../store/state";
import { Field } from "./Field";
import { CommandsFields } from "./ProjectCommands";

export type FormProps = {
  project: Project;
  state: State;
  form: ProjectForm;
  set: <K extends keyof ProjectForm>(field: K, value: ProjectForm[K]) => void;
};

/// The role names of the profile catalog for a select, keeping a saved name the catalog no longer
/// lists selectable, so saving never changes it silently.
export function profileOptions(state: State, role: string, current: string): string[] {
  const all = state.settings.profiles
    .filter((l) => l.profile.role === role && listProfile(l.profile.name, current, import.meta.env.DEV))
    .map((l) => l.profile.name);
  return current && !all.includes(current) ? [current, ...all] : all;
}

const REVIEW_MODES: { value: ReviewMode; label: string }[] = [
  { value: "human", label: "I review every finished card" },
  { value: "agent", label: "A reviewer agent reviews it" },
  { value: "both", label: "A reviewer agent first, then me" },
];
const MERGE_MODES: { value: MergeMode; label: string }[] = [
  { value: "local", label: "Merge it into the main branch" },
  { value: "github", label: "Open a pull request on GitHub" },
];
const METHODS: MergeMethod[] = ["merge", "squash", "rebase"];

/// General: which project this is (set when it was added) and its main branch.
export function GeneralSection({ project, state, form, set }: FormProps) {
  return (
    <div className="page-section" data-testid="project-general">
      <dl className="facts">
        <dt>Name</dt>
        <dd data-testid="project-name-shown">{project.name}</dd>
        <dt>Card prefix</dt>
        <dd>{project.key}</dd>
        <dt>Folder</dt>
        <dd data-testid="project-repo-shown"><code>{project.repo_path ?? "none"}</code></dd>
      </dl>
      <Field
        label="Main branch" name="work_branch" error={fieldError(state.settings, "work_branch")}
        hint="Agents start from it, and approved work lands on it."
      >
        <input data-testid="project-branch" value={form.work_branch} spellCheck={false} onChange={(e) => set("work_branch", e.target.value)} />
      </Field>
    </div>
  );
}

/// Agents: which agent runs new cards, how many at once, and the test command.
export function AgentsSection({ state, form, set }: FormProps) {
  const s = state.settings;
  return (
    <div className="page-section" data-testid="project-agents">
      <Field label="Default agent" name="default_profile" error={fieldError(s, "default_profile")} hint="Runs every card that does not name another one.">
        <select data-testid="default-profile" value={form.default_profile} onChange={(e) => set("default_profile", e.target.value)}>
          {profileOptions(state, "worker", form.default_profile).map((n) => <option key={n}>{n}</option>)}
        </select>
      </Field>
      <Field label="Agents working at once" name="max_parallel" error={fieldError(s, "max_parallel")} hint="More cards than this wait in line for a free slot.">
        <input data-testid="max-parallel" type="number" min={1} max={64} value={form.max_parallel} onChange={(e) => set("max_parallel", e.target.value)} />
      </Field>
      <Field label="Test command (optional)" name="check_command" error={fieldError(s, "check_command")} hint="Agents run it before they hand in.">
        <input data-testid="check-command" value={form.check_command} spellCheck={false} onChange={(e) => set("check_command", e.target.value)} />
      </Field>
      <CommandsFields form={form} set={set} />
    </div>
  );
}

/// Review and merge: who reviews finished cards, and what happens to an approved card.
export function ReviewSection({ state, form, set }: FormProps) {
  const s = state.settings;
  return (
    <div className="page-section" data-testid="project-review">
      <Field label="Who reviews finished cards" name="review_mode" error={fieldError(s, "review_mode")}>
        <select data-testid="review-mode" value={form.review_mode} onChange={(e) => set("review_mode", e.target.value as ReviewMode)}>
          {REVIEW_MODES.map((m) => <option key={m.value} value={m.value}>{m.label}</option>)}
        </select>
      </Field>
      <Field label="Reviewer agent" name="reviewer" error={fieldError(s, "reviewer")}>
        <select data-testid="reviewer" value={form.reviewer} onChange={(e) => set("reviewer", e.target.value)}>
          <option value="">The default for the card's agent</option>
          {profileOptions(state, "reviewer", form.reviewer).map((n) => <option key={n}>{n}</option>)}
        </select>
      </Field>
      <Field label="When a card is approved" name="merge_mode" error={fieldError(s, "merge_mode")}>
        <select data-testid="merge-mode" value={form.merge_mode} onChange={(e) => set("merge_mode", e.target.value as MergeMode)}>
          {MERGE_MODES.map((m) => <option key={m.value} value={m.value}>{m.label}</option>)}
        </select>
      </Field>
      {form.merge_mode === "github" && (
        <>
          <Field label="Pull request title" name="pr_title" error={fieldError(s, "pr_title")} hint="{key} and {title} are the card's.">
            <input data-testid="pr-title" value={form.pr_title} spellCheck={false} onChange={(e) => set("pr_title", e.target.value)} />
          </Field>
          <Field label="How to merge it" name="merge_method" error={fieldError(s, "merge_method")}>
            <select data-testid="merge-method" value={form.merge_method} onChange={(e) => set("merge_method", e.target.value as MergeMethod)}>
              {METHODS.map((m) => <option key={m}>{m}</option>)}
            </select>
          </Field>
        </>
      )}
    </div>
  );
}
