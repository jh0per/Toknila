import { useState } from "react";
import { token } from "../api/http";
import type { Design } from "../api/types";
import { type Region, fileUrl, kindLabel, placeholder, regionStyle, regionText, stepZoom } from "../store/designs";

/// A mockup large: the picture with the card's region outlined (the rest dimmed), and zoom. The
/// outline is in percent of the picture, so it stays on the same part at every zoom. A pdf shows in
/// the page, a Figma link without an image says how to get one, and a design the library lost says so.
export function DesignMockup({ project, design: d, name, region }: {
  project: string;
  /// `undefined` when the library no longer has the design.
  design: Design | undefined;
  /// The name to show when the design is gone.
  name: string;
  region: Region | null | undefined;
}) {
  const [zoom, setZoom] = useState(100);
  if (!d) {
    return <p className="muted mockup-missing" data-testid="mockup-missing">{name}: this design is no longer in the project's library.</p>;
  }
  const url = fileUrl(project, d, token());
  return (
    <figure className="mockup" data-testid="mockup" data-name={d.name}>
      <figcaption className="mockup-head">
        <strong data-testid="mockup-name">{d.name}</strong>
        <span className="muted">{kindLabel(d)}</span>
        {d.thumb && (
          <span className="mockup-zoom" role="group" aria-label="Zoom">
            <button type="button" className="btn small" data-testid="mockup-zoom-out" aria-label="Zoom out" onClick={() => setZoom(stepZoom(zoom, -1))}>−</button>
            <span data-testid="mockup-zoom-level" aria-live="polite">{zoom}%</span>
            <button type="button" className="btn small" data-testid="mockup-zoom-in" aria-label="Zoom in" onClick={() => setZoom(stepZoom(zoom, 1))}>+</button>
            <button type="button" className="btn small" data-testid="mockup-zoom-fit" onClick={() => setZoom(100)}>Fit</button>
          </span>
        )}
      </figcaption>
      {d.thumb && (
        <div className="mockup-frame" data-testid="mockup-frame">
          <div className="mockup-stage" data-testid="mockup-stage" style={{ width: `${zoom}%` }}>
            <img src={url} alt={d.name} data-testid="mockup-image" draggable={false} />
            {region && <span className="mockup-region" data-testid="mockup-region" style={regionStyle(region)} />}
          </div>
        </div>
      )}
      {d.kind === "pdf" && <iframe className="mockup-pdf" src={url} title={d.name} data-testid="mockup-pdf" />}
      {d.kind === "figma" && !d.has_file && <p className="muted">{placeholder(d)}</p>}
      {region && <p className="muted mockup-region-text" data-testid="mockup-region-text">Region: {regionText(region)}</p>}
    </figure>
  );
}
