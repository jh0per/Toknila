import type { ReactNode } from "react";
import type { TicketDetail } from "../api/types";
import { finishReport } from "./permissionReports";

/// A card in review: what the agent reported when it handed in, then the card's actions (Accept
/// or Return) right under it, so the person reads before deciding. The daemon's API gives the
/// report text; it has no check result or changed-files count for the card, so none is shown.
export function ReviewBlock({ detail, children }: { detail: TicketDetail | undefined; children: ReactNode }) {
  const report = finishReport(detail?.comments ?? []);
  return (
    <section className="review-block" data-testid="review-block">
      <h4>What the agent reported</h4>
      {report ? (
        <div className="report" data-testid="agent-report">
          <p className="report-gist">{report.body.gist}</p>
          {report.body.what && <p>{report.body.what}</p>}
          {report.body.how_to_verify && <p className="muted">How to check: {report.body.how_to_verify}</p>}
        </div>
      ) : (
        <p className="muted" data-testid="agent-report">{detail ? "The agent left no written report." : "Loading…"}</p>
      )}
      {children}
    </section>
  );
}
