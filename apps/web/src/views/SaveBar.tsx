import type { Intent } from "../store/msg";
import type { ProjectSection } from "../store/route";
import { SECTION_LABEL, sectionOfField } from "../store/projectForm";
import { formError } from "../store/settings";
import type { SettingsState } from "../store/settings";

/// The one Save bar of a settings page. It shows only when something changed (Save and Discard),
/// or to say that the last save went through, or to say what the daemon refused and where.
export function SaveBar({ project, dirty, state, section, onSave, onDiscard, dispatch }: {
  project: string;
  dirty: boolean;
  state: SettingsState;
  section: ProjectSection;
  onSave: () => void;
  onDiscard: () => void;
  dispatch: (i: Intent) => void;
}) {
  // A refusal belongs to the values that were sent; a form that is back to the saved values has none.
  const refusal = dirty && state.error?.field ? state.error : undefined;
  const general = dirty ? formError(state) : undefined;
  if (!dirty && !state.saved) return null;
  const where = refusal ? sectionOfField(refusal.field ?? "") : undefined;
  return (
    <div className="savebar" role="region" aria-label="Save changes" data-testid="savebar" data-dirty={dirty}>
      <div className="savebar-text">
        {dirty ? <span>You have unsaved changes.</span> : state.saved && !refusal && !general && (
          <span className="saved" data-testid="saved">Saved</span>
        )}
        {(refusal || general) && (
          <span className="error" role="alert" data-testid="savebar-error">
            {general ?? `${refusal?.message}`}
            {where && where !== section && (
              <>
                {" "}
                <button
                  type="button" className="link-btn"
                  onClick={() => dispatch({ type: "navigate", route: { name: "settings", project, section: where } })}
                >
                  Show in {SECTION_LABEL[where]}
                </button>
              </>
            )}
          </span>
        )}
      </div>
      {dirty && (
        <div className="savebar-btns">
          <button type="button" className="btn" disabled={state.busy} data-testid="discard-settings" onClick={onDiscard}>
            Discard
          </button>
          <button type="button" className="btn primary" disabled={state.busy} data-testid="save-settings" onClick={onSave}>
            Save changes
          </button>
        </div>
      )}
    </div>
  );
}
