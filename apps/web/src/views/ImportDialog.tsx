import { useState } from "react";
import type { ImportSource } from "../store/imports";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { Field } from "./Field";
import { FoundPlans } from "./FoundPlans";
import { ImportSummary } from "./ImportSummary";

/// Plans into a project: a path the daemon reads (a TOML chain file, a spec folder or a folder of
/// spec folders), a pasted chain file, or the plans found in the project's repository.
export function ImportDialog({ project, state, dispatch }: { project: string; state: State; dispatch: (i: Intent) => void }) {
  const [path, setPath] = useState("");
  const [toml, setToml] = useState("");
  const i = state.imports;
  const repo = state.projects.find((p) => p.key === project)?.repo_path;
  const home = repo ? `${repo.replace(/\/$/, "")}/` : "";
  const refused = (source: ImportSource) => (i.error?.source === source ? i.error.message : undefined);
  return (
    <div className="card-block form" data-testid="import-plans">
      <h2>Import into {project}</h2>
      <section className="formats" data-testid="import-formats">
        <p className="muted">Toknila reads three kinds of plans:</p>
        <ul>
          <li><b>An epic file</b> (TOML), one story per <code>[[ticket]]</code>: <code>{home}plans/login.toml</code></li>
          <li><b>A spec folder</b> with a <code>tasks.md</code> in it: <code>{home}specs/001-login</code></li>
          <li><b>A folder of spec folders</b>, one epic for each: <code>{home}specs</code></li>
        </ul>
      </section>
      <form
        className="import-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (path.trim()) dispatch({ type: "importPath", project, path: path.trim() });
        }}
      >
        <Field label="Path of an epic file or folder" name="import-path" error={refused("path")} hint="The full path, as it is on this computer.">
          <input data-testid="import-path" value={path} spellCheck={false} autoFocus onChange={(e) => setPath(e.target.value)} />
        </Field>
        <button className="btn primary" type="submit" disabled={i.busy || !path.trim()} data-testid="import-path-submit">
          Import from path
        </button>
      </form>
      <form
        className="import-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (toml.trim()) dispatch({ type: "importText", project, toml });
        }}
      >
        <Field label="Or paste an epic file" name="import-text" error={refused("paste")} hint={<>Starts with <code>[chain]</code> and a <code>name = "login"</code> line.</>}>
          <textarea data-testid="import-text" rows={4} spellCheck={false} value={toml} onChange={(e) => setToml(e.target.value)} />
        </Field>
        <button className="btn" type="submit" disabled={i.busy || !toml.trim()} data-testid="import-text-submit">
          Import pasted text
        </button>
      </form>
      {i.result && <ImportSummary result={i.result} />}
      <h3 className="sub">Plans found in this project</h3>
      <FoundPlans project={project} state={state} dispatch={dispatch} />
    </div>
  );
}
