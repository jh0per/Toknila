/// What sits over the diagram without scrolling with it: how to read the order (bottom left), the
/// Fit button and the colours of the cards (bottom right) and, while a card is being picked for
/// "Add dependency", what to click (top).
export function CanvasChrome({ fitted, cut, onFit, linking, onCancelLinking }: {
  fitted: boolean;
  /// At 100% some cards are outside the view.
  cut: boolean;
  onFit: () => void;
  /// The key of the card that will wait for the one picked, while one is being picked.
  linking: string | undefined;
  onCancelLinking: () => void;
}) {
  return (
    <>
      <p className="canvas-hint" data-testid="canvas-hint">Order is priority. Left to right, no dates.</p>
      <div className="canvas-corner">
        {cut && !fitted && <span className="canvas-cut" data-testid="cut-hint">Some cards are out of view.</span>}
        {(cut || fitted) && (
          <button
            type="button" className={`btn${cut && !fitted ? " primary" : ""}`} data-testid="fit" aria-pressed={fitted}
            title={fitted ? "Back to full size" : "Zoom out to show every card"}
            onClick={onFit}
          >
            {fitted ? "100%" : "Fit"}
          </button>
        )}
        <p className="canvas-legend" data-testid="canvas-legend" title="The colour of a card's status">
          <span className="tone-todo">todo</span>
          <span className="tone-prog">in progress</span>
          <span className="tone-attn" title="Waiting, in review, blocked or proposed: a person is needed">needs you</span>
          <span className="tone-done">done</span>
        </p>
      </div>
      {linking && (
        <p className="canvas-linking" role="status" data-testid="linking-hint">
          Click the card {linking} should wait for.
          <button type="button" className="link-btn" data-testid="linking-cancel" onClick={onCancelLinking}>Cancel</button>
        </p>
      )}
    </>
  );
}
