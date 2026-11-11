import { token } from "../api/http";
import { regionStyle } from "../store/designs";
import type { Ref } from "../store/designs";

/// A card's small thumbnail on the diagram: its first design, with the region outlined, and how
/// many more designs it has. A design that is no image (a pdf) shows no picture; the mark stays.
export function NodeThumb({ project, designs }: { project: string; designs: Ref[] }) {
  const first = designs[0];
  if (!first) return null;
  const src = `/v1/projects/${encodeURIComponent(project)}/designs/${first.design}/file?token=${encodeURIComponent(token())}`;
  return (
    <span className="node-thumb" data-testid="node-thumb" title={designs.length > 1 ? `${designs.length} designs` : "1 design"}>
      <span className="node-thumb-pic">
        <img
          src={src} alt="" draggable={false} data-testid="node-thumb-image"
          onError={(e) => { e.currentTarget.style.visibility = "hidden"; }}
        />
        {first.region && <i style={regionStyle(first.region)} />}
      </span>
      {designs.length > 1 && <b data-testid="node-thumb-more">+{designs.length - 1}</b>}
    </span>
  );
}
