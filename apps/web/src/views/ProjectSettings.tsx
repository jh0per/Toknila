import { useState } from "react";
import type { Project, ReviewMode } from "../api/types";
import type { Intent } from "../store/msg";
import { fieldError, formError } from "../store/settings";
import type { State } from "../store/state";
import { Field } from "./AddProject";

const MODES: ReviewMode[] = ["human", "agent", "both"];

/// The settings of one project, read from and saved through the daemon: default profile, parallel
/// limit, review mode and reviewer, base branch. The merge mode gets its row here with the GitHub
/// ticket; the layout keeps the place.
export function ProjectSettings({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const s = state.settings;
  const [profile, setProfile] = useState(project.default_profile);
  const [limit, setLimit] = useState(String(project.max_parallel));
  const [mode, setMode] = useState<ReviewMode>(project.default_review_mode);
  const [reviewer, setReviewer] = useState(project.default_reviewer ?? "");
  const [branch, setBranch] = useState(project.work_branch);
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
      <div className="field reserved" data-testid="merge-mode-slot">
        <span>Merge mode</span>
        <span className="muted">Chosen here once pull requests are supported.</span>
      </div>
      {formError(s) && <p className="error" role="alert">{formError(s)}</p>}
      <div className="row">
        <button className="btn primary" type="submit" disabled={s.busy} data-testid="save-settings">Save</button>
        {s.saved && <span className="saved" data-testid="saved">Saved</span>}
      </div>
    </form>
  );
}
