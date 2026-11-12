import type { RunTime, UsageFigures } from "../api/types";
import { ESTIMATE_NOTE, chipText, tokensText } from "../format";

/// How long the card's agents ran: the run in progress while one is, else all its runs together.
export const runMs = (t: RunTime | undefined, now: number): number => (t?.since != null ? Math.max(0, now - t.since) : (t?.done_ms ?? 0));

/// The small chip of a card: its estimated cost and its running time ("est. $3.80 · 2m 17s"). The
/// tokens are in its title. Nothing for a card that used none and has not run.
export function UsageChip({ figures, runTime, testId = "usage-chip" }: { figures: UsageFigures | undefined; runTime?: RunTime; testId?: string }) {
  const text = chipText(figures, runMs(runTime, Date.now()));
  if (!text) return null;
  const tokens = figures && figures.messages > 0 ? `${tokensText(figures.processed)} tokens. ` : "";
  return (
    <span className="usage-chip" data-testid={testId} title={`${tokens}${runTime?.since != null ? "Time of the current run. " : ""}${ESTIMATE_NOTE}`}>
      {text}
    </span>
  );
}
