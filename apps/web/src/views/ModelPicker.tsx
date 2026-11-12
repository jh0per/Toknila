import { useState } from "react";
import { MODEL_ALIASES } from "../store/models";

const CUSTOM = "__another__";

/// A model as one control: a select of the aliases and one entry for the default (the empty
/// value), and, once "Another model…" is chosen or the value is not an alias, a text box for a
/// full model name. `onChange` gets the model as typed ("" is the default).
export function ModelPicker({ value, onChange, testId, defaultLabel }: {
  value: string;
  onChange: (model: string) => void;
  testId: string;
  defaultLabel: string;
}) {
  const [custom, setCustom] = useState(value !== "" && !MODEL_ALIASES.includes(value));
  return (
    <>
      <select
        data-testid={testId}
        value={custom ? CUSTOM : value}
        onChange={(e) => {
          const next = e.target.value;
          setCustom(next === CUSTOM);
          if (next !== CUSTOM) onChange(next);
        }}
      >
        <option value="">{defaultLabel}</option>
        {MODEL_ALIASES.map((m) => <option key={m} value={m}>{m}</option>)}
        <option value={CUSTOM}>Another model…</option>
      </select>
      {custom && (
        <input
          data-testid={`${testId}-custom`} value={value} spellCheck={false} placeholder="A full model name"
          onChange={(e) => onChange(e.target.value)}
        />
      )}
    </>
  );
}
