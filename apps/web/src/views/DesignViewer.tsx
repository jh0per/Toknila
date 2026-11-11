import type { Design } from "../api/types";
import { token } from "../api/http";
import { fileUrl, kindLabel, placeholder } from "../store/designs";
import { Modal } from "./Modal";

/// A design large: the picture, the pdf in the page, or for a Figma link the link itself and what
/// to do about the missing image.
export function DesignViewer({ project, design: d, onClose }: { project: string; design: Design; onClose: () => void }) {
  const url = fileUrl(project, d, token());
  return (
    <Modal label={d.name} testId="design-viewer" onClose={onClose}>
      <section className="card-block design-large">
        <h2>{d.name}</h2>
        <p className="muted">{kindLabel(d)}</p>
        {d.thumb && <img src={url} alt={d.name} data-testid="design-large" />}
        {d.kind === "pdf" && <iframe src={url} title={d.name} data-testid="design-pdf" />}
        {d.kind === "figma" && d.figma && (
          <p>
            <a href={d.figma.url} target="_blank" rel="noreferrer noopener" data-testid="design-figma-link">Open the frame in Figma</a>
          </p>
        )}
        {d.kind === "figma" && !d.has_file && <p className="muted">{placeholder(d)}</p>}
      </section>
    </Modal>
  );
}
