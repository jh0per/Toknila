import { useState } from "react";
import type { Comment, ReviewView } from "../api/types";
import { ACCEPT_LINE, NOTHING_BY_HAND, RETURN_LINE, REVIEW_HINT, hintDismissed, needsYou, storeHintDismissed } from "../store/needsYou";

/// "Needs you": the PERSON items of the hand-in and of the reviewer's verdict, or the sentence that
/// says there is nothing to do by hand.
export function NeedsYou({ report, review }: { report: Comment | undefined; review: ReviewView | undefined }) {
  const items = needsYou(report, review);
  return (
    <div className="needs-you" data-testid="needs-you">
      <h5>Needs you</h5>
      {items.length === 0 ? (
        <p className="muted" data-testid="needs-you-nothing">{NOTHING_BY_HAND}</p>
      ) : (
        <ul>{items.map((t, i) => <li key={i}>{t}</li>)}</ul>
      )}
    </div>
  );
}

/// The two sentences about Accept and Return, shown above the buttons until the person dismisses
/// them (once per project, remembered in this browser).
export function ReviewHint({ project }: { project: string | undefined }) {
  const [gone, setGone] = useState(() => (project ? hintDismissed(project) : true));
  if (gone) return null;
  return (
    <div className="review-hint" data-testid="review-hint" role="note">
      <p>{REVIEW_HINT}</p>
      <button type="button" className="btn" data-testid="review-hint-dismiss" onClick={() => { if (project) storeHintDismissed(project); setGone(true); }}>Got it</button>
    </div>
  );
}

/// One line under the buttons for each: what Accept does and what Return does.
export function ReviewLines() {
  return (
    <ul className="review-lines muted" data-testid="review-lines">
      <li>{ACCEPT_LINE}</li>
      <li>{RETURN_LINE}</li>
    </ul>
  );
}
