import { useState } from "react";
import type { Design } from "../api/types";
import { token } from "../api/http";
import { fileUrl, isImage, kindLabel, placeholder } from "../store/designs";
import type { Pin } from "../store/pins";
import { Modal } from "./Modal";
import { PinList, PinMarkers } from "./PinOverlay";

const refuse = () => Promise.reject(new Error("This page cannot answer pins."));

/// A design large: the picture with the assistant's pins on it and listed under it (answered in
/// place), the pdf in the page, or for a Figma link the link itself and what to do about the
/// missing image.
export function DesignViewer({ project, design: d, onClose, pins = [], onAnswer }: {
  project: string;
  design: Design;
  onClose: () => void;
  /// The assistant's pins on this design.
  pins?: Pin[];
  onAnswer?: (id: string, text: string) => Promise<void>;
}) {
  const url = fileUrl(project, d, token());
  const [active, setActive] = useState<string | undefined>();
  const select = (id: string) => setActive(active === id ? undefined : id);
  return (
    <Modal label={d.name} testId="design-viewer" onClose={onClose}>
      <section className="card-block design-large">
        <h2>{d.name}</h2>
        <p className="muted">{kindLabel(d)}</p>
        {isImage(d) && (
          <div className="mockup-stage" data-testid="design-stage">
            <img src={url} alt={d.name} data-testid="design-large" />
            <PinMarkers pins={pins} active={active} onSelect={select} />
          </div>
        )}
        {d.kind === "pdf" && <iframe src={url} title={d.name} data-testid="design-pdf" />}
        {d.kind === "figma" && d.figma && (
          <p>
            <a href={d.figma.url} target="_blank" rel="noreferrer noopener" data-testid="design-figma-link">Open the frame in Figma</a>
          </p>
        )}
        {d.kind === "figma" && !d.has_file && <p className="muted">{placeholder(d)}</p>}
        <PinList pins={pins} active={active} onSelect={select} onAnswer={onAnswer ?? refuse} />
      </section>
    </Modal>
  );
}
