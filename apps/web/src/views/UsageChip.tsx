import type { UsageFigures } from "../api/types";
import { ESTIMATE_NOTE, chipText } from "../format";

/// The small chip of a card: tokens and estimated cost. Nothing for a card that used none.
export function UsageChip({ figures, testId = "usage-chip" }: { figures: UsageFigures | undefined; testId?: string }) {
  const text = chipText(figures);
  if (!text) return null;
  return (
    <span className="usage-chip" data-testid={testId} title={`${text}. ${ESTIMATE_NOTE}`}>
      {text}
    </span>
  );
}
