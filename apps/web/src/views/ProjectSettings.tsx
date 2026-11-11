import { useState } from "react";
import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { type ProjectForm, SECTION_LABEL, formOf, isDirty, patchOf } from "../store/projectForm";
import { PROJECT_SECTIONS, type ProjectSection } from "../store/route";
import { type State, activeProjects } from "../store/state";
import { DesignsSection } from "./DesignsSection";
import { NavLink } from "./Link";
import { PlansSection } from "./PlansSection";
import { AdvancedSection } from "./ProjectAdvanced";
import { AgentsSection, GeneralSection, ReviewSection } from "./ProjectSections";
import { SaveBar } from "./SaveBar";

type Props = { state: State; dispatch: (i: Intent) => void; project: Project | undefined; section: ProjectSection };

/// The Projects page: a picker for the project, its sections as tabs, and one Save bar for all
/// the fields of the sections that have fields.
export function ProjectSettings({ state, dispatch, project, section }: Props) {
  const projects = activeProjects(state);
  const listed = project && !projects.some((p) => p.id === project.id) ? [project, ...projects] : projects;
  return (
    <>
      <div className="picker">
        {project ? (
          <label className="picker-field">
            <span className="muted">Project</span>
            <select
              data-testid="project-picker" value={project.key}
              onChange={(e) => dispatch({ type: "navigate", route: { name: "settings", project: e.target.value, section } })}
            >
              {listed.map((p) => <option key={p.id} value={p.key}>{p.name} ({p.key})</option>)}
            </select>
          </label>
        ) : (
          <p className="muted">No project yet.</p>
        )}
        <button
          type="button" className="btn" data-testid="open-add-project"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "addProject" } })}
        >
          Add a project…
        </button>
      </div>
      {project && <Body key={formKey(project, state)} project={project} state={state} dispatch={dispatch} section={section} />}
    </>
  );
}

/// The form restarts from the daemon's values whenever they change (after a save, after a restart).
function formKey(p: Project, state: State): string {
  return [p.id, JSON.stringify(formOf(p)), state.settings.profiles.length].join("|");
}

/// The tabs and the section shown, over one form that keeps what was typed while the person looks
/// at another tab.
function Body({ project, state, dispatch, section }: Omit<Props, "project"> & { project: Project }) {
  const [form, setForm] = useState<ProjectForm>(() => formOf(project));
  const set = <K extends keyof ProjectForm>(field: K, value: ProjectForm[K]) => setForm((f) => ({ ...f, [field]: value }));
  const props = { project, state, form, set };
  const dirty = isDirty(form, project);
  const save = () => dispatch({ type: "saveProject", project: project.key, patch: patchOf(form) });
  const sections = (
    <>
      {section === "general" && <GeneralSection {...props} />}
      {section === "agents" && <AgentsSection {...props} />}
      {section === "review" && <ReviewSection {...props} />}
      {section === "advanced" && <AdvancedSection {...props} dispatch={dispatch} />}
    </>
  );
  return (
    <>
      <nav className="tabs-row" aria-label="Project settings" data-testid="settings-nav">
        {PROJECT_SECTIONS.map((s) => (
          <NavLink
            key={s} route={{ name: "settings", project: project.key, section: s }} dispatch={dispatch}
            className={`tab${section === s ? " sel" : ""}`} testId={`settings-nav-${s}`}
          >
            {SECTION_LABEL[s]}
          </NavLink>
        ))}
      </nav>
      {section === "plans" ? (
        <PlansSection project={project} state={state} dispatch={dispatch} />
      ) : section === "designs" ? (
        <DesignsSection project={project} />
      ) : (
        <form
          className="project-form" noValidate data-testid="project-form"
          onSubmit={(e) => {
            e.preventDefault();
            if (dirty) save();
          }}
        >
          {sections}
        </form>
      )}
      <SaveBar
        project={project.key} section={section} state={state.settings} dispatch={dispatch} dirty={dirty}
        onSave={save}
        onDiscard={() => {
          setForm(formOf(project));
          dispatch({ type: "dismissSettingsNotes" });
        }}
      />
    </>
  );
}
