import { useState } from "react";
import type { MergeMethod, MergeMode, Project, ReviewMode } from "../api/types";
import type { Intent } from "../store/msg";
import { MODEL_ALIASES } from "../store/models";
import { fieldError } from "../store/settings";
import type { State } from "../store/state";
import { Field } from "./AddProject";
import { SectionForm, profileOptions } from "./SectionForm";

type Props = { project: Project; state: State; dispatch: (i: Intent) => void };

const MODES: ReviewMode[] = ["human", "agent", "both"];
const MERGE_MODES: MergeMode[] = ["local", "github"];
const METHODS: MergeMethod[] = ["merge", "squash", "rebase"];

/// General: the project's name and repository (set when it was added) and its branches.
export function GeneralSection({ project, state, dispatch }: Props) {
  const s = state.settings;
  const [branch, setBranch] = useState(project.work_branch);
  return (
    <SectionForm
      project={project} title="General" testId="project-general" state={state} dispatch={dispatch}
      patch={() => ({ work_branch: branch.trim() })}
    >
      <dl className="facts">
        <dt>Name</dt>
        <dd data-testid="project-name-shown">{project.name}</dd>
        <dt>Key</dt>
        <dd>{project.key}</dd>
        <dt>Repository</dt>
        <dd data-testid="project-repo-shown"><code>{project.repo_path ?? "none"}</code></dd>
        <dt>Release branch</dt>
        <dd>{project.release_branch}</dd>
      </dl>
      <Field label="Base branch (ships start from it, accepted work lands on it)" name="work_branch" error={fieldError(s, "work_branch")}>
        <input data-testid="project-branch" value={branch} spellCheck={false} onChange={(e) => setBranch(e.target.value)} />
      </Field>
    </SectionForm>
  );
}

/// One model field: free text with the documented aliases offered.
function ModelField({ label, name, testId, value, onChange, state }: {
  label: string; name: string; testId: string; value: string; onChange: (v: string) => void; state: State;
}) {
  return (
    <Field label={label} name={name} error={fieldError(state.settings, name)}>
      <input
        data-testid={testId} value={value} spellCheck={false} list="model-aliases"
        placeholder="empty: your Claude Code default" onChange={(e) => onChange(e.target.value)}
      />
    </Field>
  );
}

/// Agents: which profile runs cards, how many at once, the check command, the Claude model and
/// the planner's profile.
export function AgentsSection({ project, state, dispatch }: Props) {
  const s = state.settings;
  const [profile, setProfile] = useState(project.default_profile);
  const [limit, setLimit] = useState(String(project.max_parallel));
  const [check, setCheck] = useState(project.check_command ?? "");
  const [worker, setWorker] = useState(project.worker_model ?? "");
  const [reviewer, setReviewer] = useState(project.reviewer_model ?? "");
  const [planner, setPlanner] = useState(project.planner_model ?? "");
  const planners = state.settings.profiles.filter((l) => l.profile.role === "planner").map((l) => l.profile.name);
  return (
    <SectionForm
      project={project} title="Agents" testId="project-agents" state={state} dispatch={dispatch}
      patch={() => ({
        default_profile: profile, max_parallel: Number(limit), check_command: check.trim(),
        worker_model: worker.trim(), reviewer_model: reviewer.trim(), planner_model: planner.trim(),
      })}
    >
      <Field label="Default profile" name="default_profile" error={fieldError(s, "default_profile")}>
        <select data-testid="default-profile" value={profile} onChange={(e) => setProfile(e.target.value)}>
          {profileOptions(state, "worker", profile).map((n) => <option key={n}>{n}</option>)}
        </select>
      </Field>
      <Field label="Agents working at once" name="max_parallel" error={fieldError(s, "max_parallel")}>
        <input data-testid="max-parallel" type="number" min={1} max={64} value={limit} onChange={(e) => setLimit(e.target.value)} />
      </Field>
      <Field label="Check command (agents run it before they hand in)" name="check_command" error={fieldError(s, "check_command")}>
        <input data-testid="check-command" value={check} spellCheck={false} placeholder="cargo xtask verify" onChange={(e) => setCheck(e.target.value)} />
      </Field>
      <ModelField label="Claude model of card agents" name="worker_model" testId="worker-model" value={worker} onChange={setWorker} state={state} />
      <ModelField label="Claude model of reviewers" name="reviewer_model" testId="reviewer-model" value={reviewer} onChange={setReviewer} state={state} />
      <ModelField label="Claude model of the planner" name="planner_model" testId="planner-model" value={planner} onChange={setPlanner} state={state} />
      <datalist id="model-aliases">{MODEL_ALIASES.map((m) => <option key={m} value={m} />)}</datalist>
      <p className="muted" data-testid="model-aliases">
        Empty uses your Claude Code default. Passed to Claude Code as <code>--model</code>, only for profiles that run it.
        Aliases: {MODEL_ALIASES.join(", ")}; a full model name works too.
      </p>
      <Field label="Planner profile (used when you start the planner)" name="planner_profile" error={undefined}>
        <select
          data-testid="settings-planner-profile" value={state.planner.profile}
          onChange={(e) => dispatch({ type: "pickPlannerProfile", profile: e.target.value })}
        >
          {(planners.includes(state.planner.profile) ? planners : [state.planner.profile, ...planners]).map((n) => <option key={n}>{n}</option>)}
        </select>
      </Field>
    </SectionForm>
  );
}

/// Review and merge: who reviews finished cards, and where accepted work lands.
export function ReviewSection({ project, state, dispatch }: Props) {
  const s = state.settings;
  const [mode, setMode] = useState<ReviewMode>(project.default_review_mode);
  const [reviewer, setReviewer] = useState(project.default_reviewer ?? "");
  const [mergeMode, setMergeMode] = useState<MergeMode>(project.merge.mode);
  const [prTitle, setPrTitle] = useState(project.merge.pr_title);
  const [method, setMethod] = useState<MergeMethod>(project.merge.method);
  return (
    <SectionForm
      project={project} title="Review and merge" testId="project-review" state={state} dispatch={dispatch}
      patch={() => ({ review_mode: mode, reviewer, merge_mode: mergeMode, pr_title: prTitle, merge_method: method })}
    >
      <Field label="Who reviews finished cards" name="review_mode" error={undefined}>
        <select data-testid="review-mode" value={mode} onChange={(e) => setMode(e.target.value as ReviewMode)}>
          {MODES.map((m) => <option key={m}>{m}</option>)}
        </select>
      </Field>
      <Field label="Reviewer profile" name="reviewer" error={fieldError(s, "reviewer")}>
        <select data-testid="reviewer" value={reviewer} onChange={(e) => setReviewer(e.target.value)}>
          <option value="">(the worker profile's own reviewer)</option>
          {profileOptions(state, "reviewer", reviewer).map((n) => <option key={n}>{n}</option>)}
        </select>
      </Field>
      <Field label="Accepted work lands" name="merge_mode" error={fieldError(s, "merge_mode")}>
        <select data-testid="merge-mode" value={mergeMode} onChange={(e) => setMergeMode(e.target.value as MergeMode)}>
          {MERGE_MODES.map((m) => <option key={m} value={m}>{m === "local" ? "local (merge into the base branch)" : "github (open a pull request)"}</option>)}
        </select>
      </Field>
      {mergeMode === "github" && (
        <>
          <Field label="Pull request title ({key} and {title})" name="pr_title" error={fieldError(s, "pr_title")}>
            <input data-testid="pr-title" value={prTitle} spellCheck={false} onChange={(e) => setPrTitle(e.target.value)} />
          </Field>
          <Field label="Merge method" name="merge_method" error={fieldError(s, "merge_method")}>
            <select data-testid="merge-method" value={method} onChange={(e) => setMethod(e.target.value as MergeMethod)}>
              {METHODS.map((m) => <option key={m}>{m}</option>)}
            </select>
          </Field>
        </>
      )}
    </SectionForm>
  );
}
