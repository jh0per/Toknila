/// The choice "Show me a design before the code", off unless the person ticks it: the card's agent
/// then opens a design (a diagram or a mockup) in Threads and waits for approval before it writes code.
export function DesignFirst({ checked, onChange }: { checked: boolean; onChange: (on: boolean) => void }) {
  return (
    <label className="check design-first">
      <input type="checkbox" data-testid="card-design-first" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>
        Show me a design before the code
        <span className="muted"> The agent first shows a diagram or a mockup for you to approve, then writes the code.</span>
      </span>
    </label>
  );
}
