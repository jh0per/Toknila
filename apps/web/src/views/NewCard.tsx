import { useState } from "react";
import type { ChainDetail } from "../api/types";
import { acceptanceFrom, cardTitle, titleFromTask } from "../store/cardForm";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import { type State, ticketsInOrder } from "../store/state";
import { DesignFirst } from "./DesignFirst";
import { Field } from "./Field";

/// A new card in a chain. It starts with what the agent should do (several lines; it is the
/// agent's prompt), then what must hold when it is done (one item per line); the title follows
/// from the first line until the person writes one. It goes at the end of the chain unless the
/// person picks the card it follows.
export function NewCard({ chain, state, dispatch }: { chain: ChainDetail; state: State; dispatch: (i: Intent) => void }) {
  const [task, setTask] = useState("");
  const [acceptance, setAcceptance] = useState("");
  // What the person typed into Title; undefined while it still follows the task.
  const [typed, setTyped] = useState<string | undefined>(undefined);
  const [after, setAfter] = useState("");
  const [designFirst, setDesignFirst] = useState(false);
  const p = state.plan;
  const refused = planError(p, "card");
  const title = typed ?? titleFromTask(task);
  return (
    <form
      className="card-block form"
      data-testid="new-card"
      noValidate
      onSubmit={(e) => {
        e.preventDefault();
        const place = after ? { after: [after] } : { at_end: true };
        dispatch({
          type: "createCard",
          chain: chain.chain.id,
          input: { title: cardTitle(typed, task), prompt: task.trim(), acceptance: acceptanceFrom(acceptance), ...(designFirst ? { design_first: true } : {}), ...place },
        });
      }}
    >
      <h2>New card in {chain.chain.name}</h2>
      <Field label="What should the agent do?" name="card-prompt" error={undefined} hint="Say it the way you would to a colleague. Several lines are fine.">
        <textarea data-testid="card-prompt" rows={5} value={task} autoFocus onChange={(e) => setTask(e.target.value)} />
      </Field>
      <Field label="How will you know it is done? (optional)" name="card-acceptance" error={undefined} hint="One item per line.">
        <textarea data-testid="card-acceptance" rows={3} value={acceptance} onChange={(e) => setAcceptance(e.target.value)} />
      </Field>
      <Field label="Title" name="card-title" error={planError(p, "card", "title")} hint="Taken from the first line; change it if you like.">
        <input data-testid="card-title" value={title} onChange={(e) => setTyped(e.target.value)} />
      </Field>
      <Field label="Comes after" name="card-after" error={undefined}>
        <select data-testid="card-after" value={after} onChange={(e) => setAfter(e.target.value)}>
          <option value="">The end of the chain</option>
          {ticketsInOrder(chain).map((t) => <option key={t.id} value={t.key}>{t.key} · {t.title}</option>)}
        </select>
      </Field>
      <DesignFirst checked={designFirst} onChange={setDesignFirst} />
      {refused && <p className="error" role="alert" data-testid="card-error">{refused}</p>}
      <button className="btn primary" type="submit" disabled={p.busy} data-testid="create-card">Add card</button>
    </form>
  );
}
