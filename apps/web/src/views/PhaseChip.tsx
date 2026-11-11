import type { Phase } from "../store/phase";
import { phaseTone } from "../store/phase";

/// A coloured dot for a phase; an empty chain has none.
export function PhaseDot({ phase }: { phase: Phase }) {
  return <span className={`phase-dot tone-${phaseTone(phase)}`} aria-hidden="true" data-empty={phase === "empty" || undefined} />;
}

/// The chain header's chip: the phase word in the phase's colour. Always shown.
export function PhaseChip({ phase }: { phase: Phase }) {
  return (
    <span className={`pill tone-${phaseTone(phase)}`} data-testid="chain-phase" data-phase={phase}>
      <PhaseDot phase={phase} />
      {phase}
    </span>
  );
}
