import type { ReactNode } from "react";

/// A labelled form field: the label, an optional hint under it, the control, and the daemon's
/// refusal of that field when there is one.
export function Field({ label, name, error, hint, children }: {
  label: string;
  name: string;
  error: string | undefined;
  hint?: ReactNode;
  children: ReactNode;
}) {
  return (
    <label className="field">
      <span className="field-label">{label}</span>
      {hint && <span className="field-hint">{hint}</span>}
      {children}
      {error && <span className="field-error" role="alert" data-testid={`error-${name}`}>{error}</span>}
    </label>
  );
}
