import { type DragEvent, useEffect, useRef, useState } from "react";
import { refusalOf, uploading } from "../store/attach";
import { get } from "../api/http";
import type { Design, DesignList, Ticket, TicketDetail } from "../api/types";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import { type Ref, designOf, withRef } from "../store/designs";
import { type Pin, pinVersion, pinsFor } from "../store/pins";
import { type State, currentProject } from "../store/state";
import { usePins } from "./usePins";
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
  const { pins, answer } = usePins(project, pinVersion(state.events));
  const cardKey = card.key;
  useEffect(() => {
    if (!project || !canEdit) return;
    const onPaste = (e: ClipboardEvent) => {
      const files = pastedImages(e.clipboardData);
      if (files.length === 0 || typing(e.target)) return;
      e.preventDefault();
      dispatch({ type: "attachFiles", project, card: cardKey, files, pasted: true });
    };
    document.addEventListener("paste", onPaste);
    return () => document.removeEventListener("paste", onPaste);
  }, [project, canEdit, cardKey, dispatch]);
  if (!project || (refs.length === 0 && !canEdit)) return null;
  const save = (next: Ref[]) => dispatch({ type: "saveCard", key: card.key, patch: { designs: next } });
  const attach = (files: File[], pasted?: boolean) => dispatch({ type: "attachFiles", project, card: card.key, files, pasted });
  return (
    <CardDesignsView
      project={project} library={library} refs={refs} canEdit={canEdit} busy={state.plan.busy}
      error={planError(state.plan, "edit")} onChange={save} cardKey={card.key} pins={pins} onAnswer={answer}
      onFiles={attach} uploading={uploading(state, card.key)}
      refused={refusalOf(state, card.key)}
    />
  );
}

/// The images on the clipboard of a paste (a screenshot arrives as a file named `image.png`).
export function pastedImages(data: DataTransfer | null): File[] {
  return [...(data?.files ?? [])].filter((f) => f.type.startsWith("image/"));
}

/// Whether the keys of the paste go to a text field, which keeps them.
const typing = (t: EventTarget | null): boolean =>
  t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));

/// [`CardDesigns`] with the library and the references given: what the page draws.
export function CardDesignsView({
  project, library, refs, canEdit, busy, error, onChange, cardKey, pins, onAnswer, onFiles, uploading = false, refused = "",
}: {
  project: string;
  /// `undefined` while the library loads.
  library: Design[] | undefined;
  refs: Ref[];
  canEdit: boolean;
  busy: boolean;
  error?: string;
  onChange: (next: Ref[]) => void;
  /// The card, and the assistant's pins of the project: the pins about this card show on its mockups.
  cardKey?: string;
  pins?: Pin[];
  onAnswer?: (id: string, text: string) => Promise<void>;
  /// Files dropped or picked, to upload and attach.
  onFiles?: (files: File[]) => void;
  /// Files are uploading: the buttons wait.
  uploading?: boolean;
  /// The sentence about the files of the last drop that were refused.
  refused?: string;
}) {
  /// The reference being picked: its index (past the end: a new one).
  const [picking, setPicking] = useState<number | undefined>();
  const [over, setOver] = useState(false);
  const picker = useRef<HTMLInputElement>(null);
  const close = () => setPicking(undefined);
  const drops = canEdit && onFiles !== undefined;
  const hasFiles = (e: DragEvent) => e.dataTransfer.types.includes("Files");
  const dropProps = drops
    ? {
        onDragOver: (e: DragEvent) => { if (!hasFiles(e)) return; e.preventDefault(); setOver(true); },
        onDragLeave: (e: DragEvent) => { if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setOver(false); },
        onDrop: (e: DragEvent) => {
          if (!hasFiles(e)) return;
          e.preventDefault();
          setOver(false);
          if (!uploading) onFiles([...e.dataTransfer.files]);
        },
      }
    : {};
  return (
    <section
      className={`card-designs${over ? " drop-over" : ""}`} data-testid="card-designs" {...dropProps}
    >
      <div className="card-designs-head">
        <div role="heading" aria-level={4} className="card-designs-title">Designs</div>
        {canEdit && (
          <span className="design-actions">
            {drops && (
              <>
                <button type="button" className="btn" disabled={uploading} data-testid="card-designs-upload" onClick={() => picker.current?.click()}>
                  Upload files
                </button>
                <input
                  ref={picker} type="file" multiple hidden data-testid="card-designs-file-input"
                  accept="image/png,image/jpeg,image/webp,application/pdf,.png,.jpg,.jpeg,.webp,.pdf"
                  onChange={(e) => { onFiles([...(e.target.files ?? [])]); e.target.value = ""; }}
                />
              </>
            )}
            <button type="button" className="btn" disabled={uploading} data-testid="add-design" onClick={() => setPicking(refs.length)}>Add design</button>
          </span>
        )}
      </div>
      {uploading && <p className="muted" role="status" data-testid="card-designs-uploading">Uploading files…</p>}
      {refused && <p className="field-error" role="alert" data-testid="card-designs-refused">{refused}</p>}
      {refs.length === 0 && drops && (
        <div className="card-designs-drop" data-testid="card-designs-drop">
          {over ? "Drop to add to this card and the project's library" : "Drop a screenshot or pdf here, paste an image, or upload files."}
          <div className="muted">png, jpg, webp or pdf, up to 25 MB each.</div>
        </div>
      )}
      {refs.length === 0 && <p className="muted" data-testid="card-designs-none">No designs. Add one from the project's library.</p>}
      {library === undefined && refs.length > 0 && <p className="muted">Loading…</p>}
      {library !== undefined && refs.map((r, i) => (
        <div className="card-design" key={`${r.design}-${i}`} data-testid="card-design">
          <DesignMockup
            project={project} design={designOf(library, r)} name={r.design} region={r.region}
            pins={cardKey ? pinsFor(pins ?? [], r.design, cardKey) : []} onAnswer={onAnswer}
          />
          {canEdit && (
            <span className="design-actions">
              <button type="button" className="btn" data-testid="design-edit" disabled={uploading} onClick={() => setPicking(i)}>Change</button>
              <button type="button" className="btn" data-testid="design-unlink" disabled={busy || uploading} onClick={() => onChange(withRef(refs, i, undefined))}>Remove</button>
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
