import { useState } from "react";
import type { AnswerKind, InboxItem } from "../api/types";
import { canAllowAlways } from "../store/commands";
import type { Intent } from "../store/msg";

const ANSWER_LABEL: Record<AnswerKind, string> = {
  allow: "Allow", deny: "Deny", text: "Send", enter: "Enter", one: "1", two: "2", up: "↑", down: "↓", escape: "Esc",
};

/// What the agent asks, and the answers core offers for it: allow or deny a permission (a deny
/// may carry a comment), keys and text for an agent that waits on its screen, and for a worker's
/// question the comment that answers it. Used in the inbox and, inline, in a waiting card's Details.
export function Answer({ item, dispatch }: { item: InboxItem; dispatch: (i: Intent) => void }) {
  const [text, setText] = useState("");
  const [comment, setComment] = useState("");
  const role = item.role ?? "worker";
  const answers = item.answers ?? [];
  const send = (answer: AnswerKind, body?: string, always?: boolean) =>
    dispatch({ type: "answer", key: item.ticket, role, answer, text: body?.trim() || undefined, always });
  const keys = answers.filter((a) => a !== "text" && a !== "allow" && a !== "deny");
  const reply = answers.includes("text") ? "screen" : role === "worker" ? "comment" : undefined;
  return (
    <div className="answer" data-role={role}>
      <blockquote data-testid="question">
        {role === "reviewer" && <strong>Reviewer: </strong>}
        {item.text}
      </blockquote>
      {item.detail && (
        <details className="detail" data-testid="question-detail">
          <summary>Show the full command</summary>
          <pre>{item.detail}</pre>
        </details>
      )}
      {item.lines && item.lines.length > 0 && <pre className="screen" data-testid="screen">{item.lines.join("\n")}</pre>}
      {answers.includes("allow") && (
        <div className="btns">
          <button className="btn primary" data-testid="answer-allow" onClick={() => send("allow")}>Allow</button>
          {canAllowAlways(item) && (
            <button
              className="btn" data-testid="answer-allow-always" title="Allow this command now and next time, in this project"
              onClick={() => send("allow", undefined, true)}
            >
              Allow always
            </button>
          )}
          <input aria-label={`Why ${item.ticket} is denied`} placeholder="Comment for a denial (optional)" value={comment} onChange={(e) => setComment(e.target.value)} />
          <button className="btn danger" data-testid="answer-deny" onClick={() => send("deny", comment)}>Deny</button>
        </div>
      )}
      {keys.length > 0 && (
        <div className="btns">
          {keys.map((k) => (
            <button key={k} className="btn" data-testid={`answer-${k}`} onClick={() => send(k)}>{ANSWER_LABEL[k]}</button>
          ))}
        </div>
      )}
      {reply === "screen" ? (
        <form
          className="compose"
          onSubmit={(e) => {
            e.preventDefault();
            if (!text.trim()) return;
            send("text", text);
            setText("");
          }}
        >
          <input aria-label={`Type to ${item.ticket}`} placeholder="Type to the agent's screen" value={text} onChange={(e) => setText(e.target.value)} />
          <button className="btn primary" type="submit" disabled={!text.trim()}>Send</button>
        </form>
      ) : reply === "comment" ? (
        <form
          className="compose"
          onSubmit={(e) => {
            e.preventDefault();
            if (!text.trim()) return;
            dispatch({ type: "addComment", key: item.ticket, kind: "instruction", text: text.trim() });
            setText("");
          }}
        >
          <textarea aria-label={`Reply to ${item.ticket}`} placeholder="Answer the agent" value={text} onChange={(e) => setText(e.target.value)} />
          <button className="btn primary" type="submit" disabled={!text.trim()}>Reply</button>
        </form>
      ) : null}
    </div>
  );
}
