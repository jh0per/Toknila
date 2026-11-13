import type { ReactNode } from "react";
import type { TicketDetail } from "../api/types";
import { finishReport } from "../store/permissionReports";
import { HandIn } from "./HandIn";
import { NeedsYou, ReviewHint, ReviewLines } from "./NeedsYou";

/// A card in review: what the agent reported when it handed in (what changed, the checks, what is
/// left), what needs the person by hand, then the card's actions (Accept or Return) right under
/// it, each with a line saying what it does, so the person reads before deciding. The checks are
/// the agent's own words; the daemon does not run them again here.
export function ReviewBlock({ detail, project, children }: { detail: TicketDetail | undefined; project: string | undefined; children: ReactNode }) {
  const report = finishReport(detail?.comments ?? []);
  return (
    <section className="review-block" data-testid="review-block">
      <h4>What the agent reported</h4>
      {report ? (
        <HandIn report={report} since={detail?.ticket.review?.since_return} />
      ) : (
        <p className="muted" data-testid="agent-report">{detail ? "The agent left no written report." : "Loading…"}</p>
      )}
      {detail && <NeedsYou report={report} review={detail.review} />}
      <ReviewHint project={project} />
      {children}
      <ReviewLines />
    </section>
  );
}
