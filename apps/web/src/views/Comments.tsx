import { useState } from "react";
import type { Comment, CommentKind, Ticket, TicketDetail } from "../api/types";
import type { Intent } from "../store/msg";
import { isPermissionReport } from "./permissionReports";

const KINDS: CommentKind[] = ["instruction", "report", "decision"];

/// The card's comments and the box to add one; an instruction is what a person tells the agent.
export function Comments(
  { card, detail, dispatch }: { card: Ticket; detail: TicketDetail | undefined; dispatch: (i: Intent) => void },
) {
  const [kind, setKind] = useState<CommentKind>("instruction");
  const [text, setText] = useState("");
  const shown = (detail?.comments ?? []).filter((c) => !isPermissionReport(c));
  const send = () => {
    if (!text.trim()) return;
    dispatch({ type: "addComment", key: card.key, kind, text: text.trim() });
    setText("");
  };
  return (
    <section className="comments" data-testid="comments">
      <h4>Comments</h4>
      {!detail ? <p className="muted">Loading…</p> : shown.length === 0 ? <p className="muted">None.</p> : (
        <ul>{shown.map((c) => <CommentItem key={c.id} c={c} />)}</ul>
      )}
      <form className="compose" onSubmit={(e) => { e.preventDefault(); send(); }}>
        <select aria-label="Kind" value={kind} onChange={(e) => setKind(e.target.value as CommentKind)}>
          {KINDS.map((k) => <option key={k} value={k}>{k}</option>)}
        </select>
        <textarea aria-label="Comment" placeholder={`Write an ${kind === "instruction" ? "instruction for the agent" : kind}`} value={text} onChange={(e) => setText(e.target.value)} />
        <button type="submit" disabled={!text.trim()}>Send {kind}</button>
      </form>
    </section>
  );
}

function CommentItem({ c }: { c: Comment }) {
  return (
    <li data-kind={c.kind}>
      <span className="kind">{c.kind}</span> <b>{c.body.gist}</b> <span className="muted">{c.author}</span>
      {c.body.what && <div>{c.body.what}</div>}
      {c.body.why && <div className="muted">{c.body.why}</div>}
    </li>
  );
}
