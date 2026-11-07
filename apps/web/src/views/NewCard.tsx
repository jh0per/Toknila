import { useState } from "react";
import type { ChainDetail } from "../api/types";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import { type State, ticketsInOrder } from "../store/state";
import { Field } from "./AddProject";

/// A new card in a chain: a title and a description in Markdown (the agent's prompt). It goes at
/// the end of the chain unless the person picks the card it follows.
export function NewCard({ chain, state, dispatch }: { chain: ChainDetail; state: State; dispatch: (i: Intent) => void }) {
  const [title, setTitle] = useState("");
  const [prompt, setPrompt] = useState("");
  const [after, setAfter] = useState("");
  const p = state.plan;
  const refused = planError(p, "card");
  return (
    <form
      className="card-block form"
      data-testid="new-card"
      noValidate
      onSubmit={(e) => {
        e.preventDefault();
        const place = after ? { after: [after] } : { at_end: true };
        dispatch({ type: "createCard", chain: chain.chain.id, input: { title: title.trim(), prompt, ...place } });
      }}
    >
      <h2>New card in {chain.chain.name}</h2>
      <Field label="Title" name="card-title" error={planError(p, "card", "title")}>
        <input data-testid="card-title" value={title} autoFocus onChange={(e) => setTitle(e.target.value)} />
      </Field>
      <Field label="Description (Markdown; what the agent is asked to do)" name="card-prompt" error={undefined}>
        <textarea data-testid="card-prompt" rows={6} value={prompt} onChange={(e) => setPrompt(e.target.value)} />
      </Field>
      <Field label="Comes after" name="card-after" error={undefined}>
        <select data-testid="card-after" value={after} onChange={(e) => setAfter(e.target.value)}>
          <option value="">the end of the chain</option>
          {ticketsInOrder(chain).map((t) => <option key={t.id} value={t.key}>{t.key} · {t.title}</option>)}
        </select>
      </Field>
      {refused && <p className="error" role="alert" data-testid="card-error">{refused}</p>}
      <button className="btn primary" type="submit" disabled={p.busy} data-testid="create-card">Add card</button>
    </form>
  );
}
