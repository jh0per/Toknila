import { useState } from "react";
import type { InboxItem, InboxKind } from "../api/types";
import type { Intent } from "../store/msg";
import type { Route } from "../store/route";
import type { State } from "../store/state";
import { itemRoute } from "../store/inbox";
import { Answer } from "./Answer";
import { NavLink } from "./Link";

/// The text of the item for a reviewer that gave no verdict (core's `NO_VERDICT_TEXT`).
const NO_VERDICT = "The reviewer gave no verdict";

/// The second line of a row: what is asked of the person, in words.
const KIND_LINE: Record<InboxKind, string> = {
  proposal: "A card the planner proposes: approve it or turn it down",
  question: "The agent is waiting for your answer",
  blocked: "Blocked: it cannot go on by itself",
  review: "Ready for your review",
  thread: "A design thread has a new version",
  leftovers: "Subtasks were left open behind a finished card",
};

type Dispatch = (i: Intent) => void;

/// One thing that waits on a person: the card's title first, what is asked as a muted second
/// line, the actions that settle it, and Open card.
export function InboxRow({ state, item, dispatch }: { state: State; item: InboxItem; dispatch: Dispatch }) {
  const route = itemRoute(state, item);
  const open = route && (
    <NavLink route={route} dispatch={dispatch} className="btn" testId="inbox-open-card">Open card</NavLink>
  );
  return (
    <li data-kind={item.kind} data-key={item.ticket}>
      <div className="row-head">
        <span className="t">{item.title}</span>
        {route ? <NavLink route={route} dispatch={dispatch} className="key">{item.ticket}</NavLink> : <span className="key">{item.ticket}</span>}
      </div>
      <div className="row-kind muted" data-testid="inbox-kind">{KIND_LINE[item.kind]}</div>
      <Body item={item} route={route} open={open} dispatch={dispatch} />
    </li>
  );
}

type BodyProps = { item: InboxItem; open: React.ReactNode; dispatch: Dispatch };

function Body({ item, route, open, dispatch }: BodyProps & { route: Route | undefined }) {
  switch (item.kind) {
    case "proposal": return <Proposal item={item} open={open} dispatch={dispatch} />;
    case "question": return <Question item={item} open={open} dispatch={dispatch} />;
    case "blocked":
      return (
        <div className="row-body">
          <span className="muted">{item.text}</span>
          <div className="btns">
            {item.allowed_actions.includes("unblock") && (
              <button className="btn primary" onClick={() => dispatch({ type: "ticketAction", key: item.ticket, action: "unblock" })}>Unblock</button>
            )}
            {item.link && <a className="btn" href={item.link} target="_blank" rel="noreferrer" data-testid="pr-link">Open the pull request</a>}
            {open}
          </div>
        </div>
      );
    case "review": return <Review item={item} open={open} dispatch={dispatch} />;
    case "thread":
      return (
        <div className="row-body">
          <span className="muted">Thread #{item.thread}: {item.text}</span>
          <div className="btns">
            {route && (
              <button
                className="btn primary"
                onClick={() => {
                  dispatch({ type: "navigate", route });
                  dispatch({ type: "selectTab", tab: "threads" });
                }}
              >
                Open the thread
              </button>
            )}
            {open}
          </div>
        </div>
      );
    case "leftovers": return <Leftovers item={item} open={open} dispatch={dispatch} />;
  }
}

function Proposal({ item, open, dispatch }: BodyProps) {
  const [rejecting, setRejecting] = useState(false);
  const [text, setText] = useState("");
  return (
    <div className="row-body">
      <div className="btns">
        {item.allowed_actions.includes("approve") && (
          <button className="btn primary" onClick={() => dispatch({ type: "ticketAction", key: item.ticket, action: "approve" })}>
            Approve
          </button>
        )}
        {item.allowed_actions.includes("reject") && (
          <button className="btn danger" onClick={() => setRejecting(!rejecting)}>Reject</button>
        )}
        {open}
      </div>
      {rejecting && (
        <form
          className="compose"
          onSubmit={(e) => {
            e.preventDefault();
            if (text.trim()) dispatch({ type: "ticketAction", key: item.ticket, action: "reject", text: text.trim() });
          }}
        >
          <textarea aria-label={`Why ${item.ticket} is turned down`} placeholder="Why is it turned down?" value={text} onChange={(e) => setText(e.target.value)} />
          <button className="btn danger" type="submit" disabled={!text.trim()}>Reject {item.ticket}</button>
        </form>
      )}
    </div>
  );
}

/// A question: what the agent asks and how it can be answered, then Open card.
function Question({ item, open, dispatch }: BodyProps) {
  return (
    <div className="row-body">
      <Answer item={item} dispatch={dispatch} />
      <div className="btns">{open}</div>
    </div>
  );
}

/// A card for the person to review: Accept, Return (with what to change), and, when the reviewer
/// gave no verdict, the end of its screen and Run the reviewer again. The same actions as the card.
function Review({ item, open, dispatch }: BodyProps) {
  const [returning, setReturning] = useState(false);
  const [comment, setComment] = useState("");
  const act = (action: "accept" | "run_reviewer") => dispatch({ type: "ticketAction", key: item.ticket, action });
  const noVerdict = item.text === NO_VERDICT;
  const can = (a: InboxItem["allowed_actions"][number]) => item.allowed_actions.includes(a);
  return (
    <div className="row-body" data-testid="no-verdict" data-no-verdict={noVerdict}>
      {(noVerdict || item.text) && <blockquote data-testid="question">{item.text}</blockquote>}
      {item.lines && item.lines.length > 0 && <pre className="screen" data-testid="screen">{item.lines.join("\n")}</pre>}
      <div className="btns">
        {can("accept") && <button className="btn primary" data-testid="review-accept" onClick={() => act("accept")}>Accept</button>}
        {can("return") && (
          <button className="btn" data-testid="review-return-open" onClick={() => setReturning(!returning)}>Return…</button>
        )}
        {can("run_reviewer") && <button className="btn" data-testid="review-run-again" onClick={() => act("run_reviewer")}>Run the reviewer again</button>}
        {open}
      </div>
      {can("return") && returning && (
        <form
          className="compose"
          onSubmit={(e) => {
            e.preventDefault();
            if (!comment.trim()) return;
            dispatch({ type: "ticketAction", key: item.ticket, action: "return", text: comment.trim() });
            setComment("");
          }}
        >
          <input aria-label={`What ${item.ticket} needs`} placeholder="What should the agent change?" autoFocus value={comment} onChange={(e) => setComment(e.target.value)} />
          <button className="btn primary" type="submit" data-testid="review-return" disabled={!comment.trim()}>Send back</button>
        </form>
      )}
    </div>
  );
}

function Leftovers({ item, open, dispatch }: BodyProps) {
  const decide = (action: "move" | "close" | "leave") => dispatch({ type: "leftovers", key: item.ticket, action });
  return (
    <div className="row-body">
      <p className="muted">
        {item.done_cards.join(", ")} {item.done_cards.length === 1 ? "was" : "were"} done before this card; these subtasks are still open:
      </p>
      <ul className="ck">{item.subtasks.map((s, i) => <li key={i}>{s}</li>)}</ul>
      <div className="btns">
        <button className="btn primary" onClick={() => decide("move")}>Move to a new card</button>
        <button className="btn" onClick={() => decide("close")}>Close them</button>
        <button className="btn" onClick={() => decide("leave")}>Leave as is</button>
        {open}
      </div>
    </div>
  );
}
