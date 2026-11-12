import type { CommandPolicy } from "../api/types";
import { COMMAND_CHOICES, ruleWords } from "../store/commands";
import { fieldError } from "../store/settings";
import { Field } from "./Field";
import type { FormProps } from "./ProjectSections";

/// The setup command: run once in every new workspace, before the agent starts.
export function SetupCommandField({ state, form, set }: Pick<FormProps, "state" | "form" | "set">) {
  return (
    <Field
      label="Setup command (optional)" name="setup_command" error={fieldError(state.settings, "setup_command")}
      hint="Runs once in every new workspace before the agent starts, for example npm ci --prefix apps/web."
    >
      <input data-testid="setup-command" value={form.setup_command} spellCheck={false} onChange={(e) => set("setup_command", e.target.value)} />
    </Field>
  );
}

/// Agents: when a card's agent wants to run a command (three plain choices with one line each),
/// and the commands the person allowed always, each with a button that forgets it. Both belong to
/// the form, so they are saved with the Save bar like every other field.
export function CommandsFields({ form, set }: Pick<FormProps, "form" | "set">) {
  return (
    <>
      <fieldset className="choices" data-testid="command-policy">
        <legend className="field-label">When the agent wants to run a command</legend>
        {COMMAND_CHOICES.map((c) => (
          <label key={c.value} className="choice">
            <input
              type="radio" name="command_policy" value={c.value} data-testid={`command-policy-${c.value}`}
              checked={form.command_policy === c.value}
              onChange={() => set("command_policy", c.value as CommandPolicy)}
            />
            <span>
              <strong>{c.label}</strong>
              <span className="field-hint">{c.hint}</span>
            </span>
          </label>
        ))}
      </fieldset>
      <div className="field" data-testid="allowed-commands">
        <span className="field-label">Commands you allowed always</span>
        {form.allowed_commands.length === 0 ? (
          <span className="field-hint" data-testid="allowed-none">
            None yet. Choose Allow always on a question about a command and it is listed here.
          </span>
        ) : (
          <ul className="rules">
            {form.allowed_commands.map((rule, i) => (
              <li key={rule}>
                <code>{ruleWords(rule)}</code>
                <button
                  type="button" className="btn" data-testid={`forget-rule-${i}`} aria-label={`Forget ${ruleWords(rule)}`}
                  onClick={() => set("allowed_commands", form.allowed_commands.filter((r) => r !== rule))}
                >
                  Remove
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </>
  );
}
