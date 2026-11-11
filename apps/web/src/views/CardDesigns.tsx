import { useEffect, useState } from "react";
import { get } from "../api/http";
import type { Design, DesignList, Ticket, TicketDetail } from "../api/types";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import { type Ref, designOf, withRef } from "../store/designs";
import { type State, currentProject } from "../store/state";
import { DesignMockup } from "./DesignMockup";
import { DesignPicker } from "./DesignPicker";

/// The designs a card references, on its page: each mockup large with its region outlined, and,
/// while the daemon lets the card be edited, the picker to add one, change its region or remove it.
/// The project's library is read here; the references are saved through the card's own patch.
export function CardDesigns({ state, card, detail, dispatch }: {
  state: State;
  card: Ticket;
  detail: TicketDetail | undefined;
  dispatch: (i: Intent) => void;
}) {
  const project = currentProject(state)?.key;
  const [library, setLibrary] = useState<Design[] | undefined>();
  const refs = card.designs ?? [];
  const canEdit = Boolean(detail?.can_edit);
  useEffect(() => {
    if (!project || (refs.length === 0 && !canEdit)) return;
    let live = true;
    get<DesignList>(`/projects/${encodeURIComponent(project)}/designs`)
      .then((l) => live && setLibrary(l.designs))
      .catch(() => live && setLibrary([]));
    return () => { live = false; };
  }, [project, refs.length, canEdit]);
  if (!project || (refs.length === 0 && !canEdit)) return null;
  const save = (next: Ref[]) => dispatch({ type: "saveCard", key: card.key, patch: { designs: next } });
  return (
    <CardDesignsView
      project={project} library={library} refs={refs} canEdit={canEdit} busy={state.plan.busy}
      error={planError(state.plan, "edit")} onChange={save}
    />
  );
}

/// [`CardDesigns`] with the library and the references given: what the page draws.
export function CardDesignsView({ project, library, refs, canEdit, busy, error, onChange }: {
  project: string;
  /// `undefined` while the library loads.
  library: Design[] | undefined;
  refs: Ref[];
  canEdit: boolean;
  busy: boolean;
  error?: string;
  onChange: (next: Ref[]) => void;
}) {
  /// The reference being picked: its index (past the end: a new one).
  const [picking, setPicking] = useState<number | undefined>();
  const close = () => setPicking(undefined);
  return (
    <section className="card-designs" data-testid="card-designs">
      <div className="card-designs-head">
        <div role="heading" aria-level={4} className="card-designs-title">Designs</div>
        {canEdit && (
          <button type="button" className="btn" data-testid="add-design" onClick={() => setPicking(refs.length)}>Add design</button>
        )}
      </div>
      {refs.length === 0 && <p className="muted" data-testid="card-designs-none">No designs. Add one from the project's library.</p>}
      {library === undefined && refs.length > 0 && <p className="muted">Loading…</p>}
      {library !== undefined && refs.map((r, i) => (
        <div className="card-design" key={`${r.design}-${i}`} data-testid="card-design">
          <DesignMockup project={project} design={designOf(library, r)} name={r.design} region={r.region} />
          {canEdit && (
            <span className="design-actions">
              <button type="button" className="btn" data-testid="design-edit" onClick={() => setPicking(i)}>Change</button>
              <button type="button" className="btn" data-testid="design-unlink" disabled={busy} onClick={() => onChange(withRef(refs, i, undefined))}>Remove</button>
            </span>
          )}
        </div>
      ))}
      {error && picking === undefined && <p className="field-error" role="alert" data-testid="card-designs-error">{error}</p>}
      {picking !== undefined && library && (
        <DesignPicker
          project={project} library={library} initial={refs[picking]} busy={busy} error={error}
          onClose={close}
          onSave={(ref) => { onChange(withRef(refs, picking, ref)); close(); }}
        />
      )}
    </section>
  );
}
