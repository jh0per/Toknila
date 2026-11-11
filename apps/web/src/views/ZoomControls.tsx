import { MAX_ZOOM, MIN_ZOOM } from "./zoomMath";

/// The zoom buttons and Fit of a zoomable view (the diagram, the designs board): the same
/// controls, labels and test ids on both.
export function ZoomControls({ zoom, fitted, cut, onFit, onZoomIn, onZoomOut, what }: {
  zoom: number;
  fitted: boolean;
  /// At the current zoom something is outside the view.
  cut: boolean;
  onFit: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  /// What Fit shows, in a tooltip: "every card", "every design".
  what: string;
}) {
  return (
    <>
      <span className="canvas-zoom" role="group" aria-label="Zoom" data-testid="zoom-controls">
        <button
          type="button" className="btn" data-testid="zoom-out" aria-label="Zoom out" title="Zoom out (Cmd -)"
          disabled={zoom <= MIN_ZOOM} onClick={onZoomOut}
        >
          −
        </button>
        <span className="canvas-zoom-value" data-testid="zoom-value" title="Cmd 0 for 100%">{Math.round(zoom * 100)}%</span>
        <button
          type="button" className="btn" data-testid="zoom-in" aria-label="Zoom in" title="Zoom in (Cmd +)"
          disabled={zoom >= MAX_ZOOM} onClick={onZoomIn}
        >
          +
        </button>
      </span>
      <button
        type="button" className={`btn${cut && !fitted ? " primary" : ""}`} data-testid="fit" aria-pressed={fitted}
        title={fitted ? "Back to full size" : `Zoom out to show ${what}`}
        onClick={onFit}
      >
        {fitted ? "100%" : "Fit"}
      </button>
    </>
  );
}
