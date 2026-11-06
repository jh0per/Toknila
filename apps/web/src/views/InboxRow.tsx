import { useState } from "react";
import type { AnswerKind, InboxItem } from "../api/types";
import type { Intent } from "../store/msg";
import type { Route } from "../store/route";
import type { State } from "../store/state";
import { itemRoute } from "../store/inbox";
import { NavLink } from "./Link";

/// One thing that waits on a person, with the action that settles it inline.
export function InboxRow({ state, item, dispatch }: { state: State; item: InboxItem; dispatch: (i: Intent) => void }) {
  const route = itemRoute(state, item);
  return (
    <li data-kind={item.kind} data-key={item.ticket}>
      <div className="row-head">
        {route ? <NavLink route={route} dispatch={dispatch} className="key">{item.ticket}</NavLink> : <span className="key">{item.ticket}</span>}
        <span className="t">{item.title}</span>
      </div>
      <Body item={item} route={route} dispatch={dispatch} />
    </li>
  );
}

function Body({ item, route, dispatch }: { item: InboxItem; route: Route | undefined; dispatch: (i: Intent) => void }) {
  switch (item.kind) {
    case "proposal": return <Proposal item={item} dispatch={dispatch} />;
    case "question": return <Question item={item} dispatch={dispatch} />;
    case "blocked":
      return (
        <div className="row-body">
          <span className="muted">{item.text}</span>
          {item.link && <a href={item.link} target="_blank" rel="noreferrer" data-testid="pr-link">Open the pull request</a>}
          {item.allowed_actions.includes("unblock") && (
            <button onClick={() => dispatch({ type: "ticketAction", key: item.ticket, action: "unblock" })}>Unblock</button>
          )}
        </div>
      );
    case "review": return <p className="muted">Your turn to review. Open the card to accept or return it.</p>;
    case "thread":
      return (
        <div className="row-body">
          <span className="muted">Thread #{item.thread}: {item.text} has a new version.</span>
          {route && (
            <button
              onClick={() => {
                dispatch({ type: "navigate", route });
                dispatch({ type: "selectTab", tab: "threads" });
              }}
            >
              Open the thread
            </button>
          )}
        </div>
      );
    case "leftovers": return <Leftovers item={item} dispatch={dispatch} />;
  }
}

function Proposal({ item, dispatch }: { item: InboxItem; dispatch: (i: Intent) => void }) {
  const [rejecting, setRejecting] = useState(false);
  const [text, setText] = useState("");
  return (
    <div className="row-body">
      <div className="btns">
        {item.allowed_actions.includes("approve") && (
          <button className="primary" onClick={() => dispatch({ type: "ticketAction", key: item.ticket, action: "approve" })}>
            Approve
          </button>
        )}
        {item.allowed_actions.includes("reject") && (
          <button className="danger" onClick={() => setRejecting(!rejecting)}>Reject</button>
        )}
      </div>
      {rejecting && (
        <form
          className="compose"
          onSubmit={(e) => {
            e.preventDefault();
            if (text.trim()) dispatch({ type: "ticketAction", key: item.ticket, action: "reject", text: text.trim() });
          }}
        >
          <textarea aria-label={`Why ${item.ticket} is turned down`} value={text} onChange={(e) => setText(e.target.value)} />
          <button type="submit" disabled={!text.trim()}>Reject {item.ticket}</button>
        </form>
      )}
    </div>
  );
}

const ANSWER_LABEL: Record<AnswerKind, string> = {
  allow: "Allow", deny: "Deny", text: "Send", enter: "Enter", one: "1", two: "2", escape: "Esc",
};

/// What the agent asks, and the answers core offers for it: allow or deny a permission (a deny
/// may carry a comment), keys and text for an agent that waits on its screen, and for a worker's
/// question the comment that answers it.
function Question({ item, dispatch }: { item: InboxItem; dispatch: (i: Intent) => void }) {
  const [text, setText] = useState("");
  const [comment, setComment] = useState("");
  const role = item.role ?? "worker";
  const answers = item.answers ?? [];
  const send = (answer: AnswerKind, body?: string) =>
    dispatch({ type: "answer", key: item.ticket, role, answer, text: body?.trim() || undefined });
  const keys = answers.filter((a) => a !== "text" && a !== "allow" && a !== "deny");
  return (
    <div className="row-body" data-role={role}>
      <blockquote data-testid="question">
        {role === "reviewer" && <strong>Reviewer: </strong>}
        {item.text}
      </blockquote>
      {item.lines && item.lines.length > 0 && <pre className="screen" data-testid="screen">{item.lines.join("\n")}</pre>}
      {answers.includes("allow") && (
        <div className="btns">
          <button className="primary" data-testid="answer-allow" onClick={() => send("allow")}>Allow</button>
          <input aria-label={`Why ${item.ticket} is denied`} placeholder="Comment for a denial (optional)" value={comment} onChange={(e) => setComment(e.target.value)} />
          <button className="danger" data-testid="answer-deny" onClick={() => send("deny", comment)}>Deny</button>
        </div>
      )}
      {keys.length > 0 && (
        <div className="btns">
          {keys.map((k) => (
            <button key={k} data-testid={`answer-${k}`} onClick={() => send(k)}>{ANSWER_LABEL[k]}</button>
          ))}
        </div>
      )}
      {answers.includes("text") ? (
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
          <button type="submit" disabled={!text.trim()}>Send</button>
        </form>
      ) : role === "worker" ? (
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
        <button type="submit" disabled={!text.trim()}>Reply</button>
      </form>
      ) : null}
    </div>
  );
}

function Leftovers({ item, dispatch }: { item: InboxItem; dispatch: (i: Intent) => void }) {
  const decide = (action: "move" | "close" | "leave") => dispatch({ type: "leftovers", key: item.ticket, action });
  return (
    <div className="row-body">
      <p className="muted">
        {item.done_cards.join(", ")} {item.done_cards.length === 1 ? "was" : "were"} done before this card; these subtasks are still open:
      </p>
      <ul className="ck">{item.subtasks.map((s, i) => <li key={i}>{s}</li>)}</ul>
      <div className="btns">
        <button className="primary" onClick={() => decide("move")}>Move to a new card</button>
        <button onClick={() => decide("close")}>Close them</button>
        <button onClick={() => decide("leave")}>Leave as is</button>
      </div>
    </div>
  );
}
