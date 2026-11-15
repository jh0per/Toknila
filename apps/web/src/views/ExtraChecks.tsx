import type { ProjectForm } from "../store/projectForm";

/// The extra checks under the test command: one row per check (the paths it is for, and its
/// command), a button that removes a row and one that adds a row. The rows belong to the form,
/// so the Save bar saves them with every other field.
export function ExtraChecksField({ form, set, error }: {
  form: ProjectForm;
  set: (field: "extra_checks", value: ProjectForm["extra_checks"]) => void;
  error: string | undefined;
}) {
  const rows = form.extra_checks;
  const change = (i: number, part: Partial<ProjectForm["extra_checks"][number]>) =>
    set("extra_checks", rows.map((r, j) => (j === i ? { ...r, ...part } : r)));
  return (
    <fieldset className="field extra-checks" data-testid="extra-checks">
      <legend className="field-label">Extra checks (optional)</legend>
      <span className="field-hint">
        Run after the test command, in this order, when a story changed files that match the paths, for example
        apps/web/** with cargo xtask web. Separate several paths with commas.
      </span>
      {rows.map((r, i) => (
        // The rows have no identity but their place.
        // eslint-disable-next-line react/no-array-index-key
        <div key={i} className="extra-check-row" data-testid={`extra-check-${i}`}>
          <input
            data-testid={`extra-check-paths-${i}`} aria-label={`Paths of extra check ${i + 1}`} placeholder="apps/web/**"
            value={r.paths} spellCheck={false} onChange={(e) => change(i, { paths: e.target.value })}
          />
          <input
            data-testid={`extra-check-command-${i}`} aria-label={`Command of extra check ${i + 1}`} placeholder="cargo xtask web"
            value={r.command} spellCheck={false} onChange={(e) => change(i, { command: e.target.value })}
          />
          <button
            type="button" className="btn" data-testid={`remove-extra-check-${i}`} aria-label={`Remove extra check ${i + 1}`}
            onClick={() => set("extra_checks", rows.filter((_, j) => j !== i))}
          >
            Remove
          </button>
        </div>
      ))}
      <button
        type="button" className="btn" data-testid="add-extra-check"
        onClick={() => set("extra_checks", [...rows, { paths: "", command: "" }])}
      >
        Add an extra check
      </button>
      {error && <span className="field-error" role="alert" data-testid="error-extra_checks">{error}</span>}
    </fieldset>
  );
}
