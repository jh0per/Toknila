import { useState } from "react";
import type { MergeMethod, MergeMode, Project, ReviewMode } from "../api/types";
import type { Intent } from "../store/msg";
import { fieldError, formError } from "../store/settings";
import type { State } from "../store/state";
import { Field } from "./AddProject";

const MODES: ReviewMode[] = ["human", "agent", "both"];
const MERGE_MODES: MergeMode[] = ["local", "github"];
const METHODS: MergeMethod[] = ["merge", "squash", "rebase"];

/// The settings of one project, read from and saved through the daemon: default profile, parallel
/// limit, review mode and reviewer, base branch, and where accepted work lands (the local base
/// branch, or a GitHub pull request with its title template and merge method).
export function ProjectSettings({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const s = state.settings;
  const [profile, setProfile] = useState(project.default_profile);
  const [limit, setLimit] = useState(String(project.max_parallel));
  const [mode, setMode] = useState<ReviewMode>(project.default_review_mode);
  const [reviewer, setReviewer] = useState(project.default_reviewer ?? "");
  const [branch, setBranch] = useState(project.work_branch);
  const [mergeMode, setMergeMode] = useState<MergeMode>(project.merge.mode);
  const [prTitle, setPrTitle] = useState(project.merge.pr_title);
  const [method, setMethod] = useState<MergeMethod>(project.merge.method);
  const named = (role: string) => s.profiles.filter((l) => l.profile.role === role).map((l) => l.profile.name);
  // A saved name the catalog no longer lists stays selectable, so saving never changes it silently.
  const options = (role: string, current: string) => {
    const all = named(role);
    return current && !all.includes(current) ? [current, ...all] : all;
  };
  return (
    <form
      className="card-block form"
      noValidate
      data-testid="project-settings"
      onSubmit={(e) => {
        e.preventDefault();
        dispatch({
          type: "saveProject",
          project: project.key,
          patch: {
            default_profile: profile, max_parallel: Number(limit), review_mode: mode, reviewer, work_branch: branch.trim(),
            merge_mode: mergeMode, pr_title: prTitle, merge_method: method,
          },
        });
      }}
    >
      <h2>Settings of {project.key}</h2>
      <Field label="Default profile" name="default_profile" error={fieldError(s, "default_profile")}>
        <select data-testid="default-profile" value={profile} onChange={(e) => setProfile(e.target.value)}>
          {options("worker", profile).map((n) => <option key={n}>{n}</option>)}
        </select>
      </Field>
      <Field label="Agents working at once" name="max_parallel" error={fieldError(s, "max_parallel")}>
        <input
          data-testid="max-parallel" type="number" min={1} max={64} value={limit}
          onChange={(e) => setLimit(e.target.value)}
        />
      </Field>
      <Field label="Who reviews finished cards" name="review_mode" error={undefined}>
        <select data-testid="review-mode" value={mode} onChange={(e) => setMode(e.target.value as ReviewMode)}>
          {MODES.map((m) => <option key={m}>{m}</option>)}
        </select>
      </Field>
      <Field label="Reviewer profile" name="reviewer" error={fieldError(s, "reviewer")}>
        <select data-testid="reviewer" value={reviewer} onChange={(e) => setReviewer(e.target.value)}>
          <option value="">(the worker profile's own reviewer)</option>
          {options("reviewer", reviewer).map((n) => <option key={n}>{n}</option>)}
        </select>
      </Field>
      <Field label="Base branch" name="work_branch" error={fieldError(s, "work_branch")}>
        <input data-testid="project-branch" value={branch} spellCheck={false} onChange={(e) => setBranch(e.target.value)} />
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
      {formError(s) && <p className="error" role="alert">{formError(s)}</p>}
      <div className="row">
        <button className="btn primary" type="submit" disabled={s.busy} data-testid="save-settings">Save</button>
        {s.saved && <span className="saved" data-testid="saved">Saved</span>}
      </div>
    </form>
  );
}
