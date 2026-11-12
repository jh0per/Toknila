import { type PointerEvent, useRef, useState } from "react";
import { token } from "../api/http";
import type { Design } from "../api/types";
import { type Ref, type Region, dragRegion, fileUrl, regionStyle, regionText } from "../store/designs";
import { Modal } from "./Modal";

type Point = { x: number; y: number };

/// The picker of a card's design: the project's library as a grid, and, once one is chosen, its
/// picture to draw a region on (drag a rectangle; the region is shown in percent of the image).
/// With `initial` it edits an existing reference. A design that has no image (a pdf, a Figma link
/// without one) can be chosen too, for the whole.
export function DesignPicker({ project, library, initial, busy, error, onSave, onClose }: {
  project: string;
  library: Design[];
  initial?: Ref;
  busy: boolean;
  error?: string;
  onSave: (ref: Ref) => void;
  onClose: () => void;
}) {
  const [chosen, setChosen] = useState<string | undefined>(initial?.design);
  const [region, setRegion] = useState<Region | undefined>(initial?.region ?? undefined);
  const [drag, setDrag] = useState<{ from: Point; to: Point } | undefined>();
  const stage = useRef<HTMLDivElement>(null);
  const design = library.find((d) => d.id === chosen);
  const at = (e: PointerEvent): Point => {
    const box = stage.current?.getBoundingClientRect();
    if (!box || box.width === 0 || box.height === 0) return { x: 0, y: 0 };
    return { x: (e.clientX - box.left) / box.width, y: (e.clientY - box.top) / box.height };
  };
  const shown = drag ? dragRegion(drag.from, drag.to) : region;
  return (
    <Modal label="Pick a design" testId="design-picker" onClose={onClose}>
      <section className="card-block design-picker">
        <h2>{initial ? "Change the design of this story" : "Pick a design"}</h2>
        {library.length === 0 ? (
          <p className="muted" data-testid="picker-none">The project has no designs yet. Add them under Settings, Designs.</p>
        ) : (
          <ul className="design-grid picker-grid" data-testid="picker-list">
            {library.map((d) => (
              <li key={d.id}>
                <button
                  type="button" className={`design-thumb pick${d.id === chosen ? " chosen" : ""}`} data-testid="picker-item" data-name={d.name}
                  aria-pressed={d.id === chosen} aria-label={d.name}
                  onClick={() => { setChosen(d.id); setRegion(undefined); }}
                >
                  {d.thumb ? <img src={fileUrl(project, d, token())} alt="" loading="lazy" /> : <span className="design-note">{d.kind === "pdf" ? "PDF" : "Figma"}</span>}
                </button>
                <span className="design-name">{d.name}</span>
              </li>
            ))}
          </ul>
        )}
        {design?.thumb && (
          <>
            <p className="muted">Optional: drag on the image to mark the part that matters.</p>
            <div
              ref={stage} className="mockup-stage draw" data-testid="picker-stage"
              onPointerDown={(e) => { e.currentTarget.setPointerCapture?.(e.pointerId); const p = at(e); setDrag({ from: p, to: p }); }}
              onPointerMove={(e) => drag && setDrag({ from: drag.from, to: at(e) })}
              onPointerUp={(e) => {
                if (!drag) return;
                const r = dragRegion(drag.from, at(e));
                if (r) setRegion(r);
                setDrag(undefined);
              }}
            >
              <img src={fileUrl(project, design, token())} alt={design.name} draggable={false} data-testid="picker-image" />
              {shown && <span className="mockup-region" data-testid="picker-region" style={regionStyle(shown)} />}
            </div>
            <p className="muted" data-testid="picker-region-text">{region ? `Region: ${regionText(region)}` : "The whole image."}</p>
            {region && <button type="button" className="btn small" data-testid="picker-clear-region" onClick={() => setRegion(undefined)}>Clear region</button>}
          </>
        )}
        {error && <p className="field-error" role="alert" data-testid="picker-error">{error}</p>}
        <div className="plan-btns">
          <button type="button" className="btn primary" disabled={busy || !design} data-testid="picker-save" onClick={() => design && onSave({ design: design.id, region: region ?? null })}>
            {initial ? "Save" : "Add to the story"}
          </button>
          <button type="button" className="btn" data-testid="picker-cancel" onClick={onClose}>Cancel</button>
        </div>
      </section>
    </Modal>
  );
}
