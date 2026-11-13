import { useState } from "react";
import { post } from "../api/http";
import { type CardItem, type InboxItem, type InboxKind, type TicketDetail, isCardItem } from "../api/types";
import { RUNNER_LOST_TWICE } from "../format";
import { MAX_ANSWER, answerProblem } from "../store/pins";
import { finishReport } from "../store/permissionReports";
import { HandIn, InboxHandInBlocks } from "./HandIn";
import type { Intent } from "../store/msg";
import type { Route } from "../store/route";
import type { State } from "../store/state";
import { itemRoute } from "../store/inbox";
import { epicTitle } from "../store/title";
import { Answer } from "./Answer";
import { NavLink } from "./Link";

/// The text of the item for a reviewer that gave no verdict (core's `NO_VERDICT_TEXT`).
const NO_VERDICT = "The reviewer gave no verdict";

/// The second line of a row: what is asked of the person, in words.
const KIND_LINE: Record<InboxKind, string> = {
  proposal: "A story the planner proposes: approve it or turn it down",
  question: "The agent is waiting for your answer",
  blocked: "Blocked: it cannot go on by itself",
  review: "Ready for your review",
  thread: "A design thread has a new version",
  leftovers: "Tasks were left open behind a finished story",
  design_question: "The assistant asks about a design",
  pin: "The assistant asks about a design",
};

/// The second line of a row: for a blocked card the reason's own sentence (a double loss of the
/// runner says what to press), else what is asked of the person.
export function kindLine(item: Pick<InboxItem, "kind" | "text">): string {
  if (item.kind !== "blocked" || !item.text) return KIND_LINE[item.kind];
  if (item.text === RUNNER_LOST_TWICE) return item.text;
  const sentence = (item.text.split("\n")[0] ?? "").replace(/^[a-z_]+: /, "");
  return sentence ? `Blocked: ${sentence}` : KIND_LINE.blocked;
}

type Dispatch =(i: Intent) => void;

/// One thing that waits on a person: the card's title first, what is asked as a muted second
/// line, the actions that settle it, and Open card.
export function InboxRow({ state, item, dispatch }: { state: State; item: InboxItem; dispatch: Dispatch }) {
  // A pin no card owns: the design is the title, nothing to open, answered in place.
  if (!isCardItem(item)) {
    return (
      <li data-kind={item.kind} data-key={item.pin ?? ""}>
        <div className="row-head">
          <span className="t">{item.title}</span>
        </div>
        <div className="row-kind muted" data-testid="inbox-kind">The assistant asks about {item.design ?? item.title}</div>
        <DesignQuestion item={item} project={item.project ?? undefined} open={undefined} />
      </li>
    );
  }
  const route = itemRoute(state, item);
  const open = route && (
    <NavLink route={route} dispatch={dispatch} className="btn" testId="inbox-open-card">Open story</NavLink>
  );
  return (
    <li data-kind={item.kind} data-key={item.ticket}>
      <div className="row-head">
        <span className="t">{item.title}</span>
        {route ? <NavLink route={route} dispatch={dispatch} className="key">{item.ticket}</NavLink> : <span className="key">{item.ticket}</span>}
      </div>
      <div className="row-kind muted" data-testid="inbox-kind">{kindLine(item)}</div>
      {state.chains[item.chain] && <div className="row-epic muted" data-testid="inbox-epic">{epicTitle(state.chains[item.chain])}</div>}
      <Body item={item} route={route} open={open} dispatch={dispatch} detail={state.details[item.ticket]} />
    </li>
  );
}

type BodyProps = { item: CardItem; open: React.ReactNode; dispatch: Dispatch };

function Body({ item, route, open, dispatch, detail }: BodyProps & { route: Route | undefined; detail: TicketDetail | undefined }) {
  switch (item.kind) {
    case "proposal": return <Proposal item={item} open={open} dispatch={dispatch} />;
    case "question": return <Question item={item} open={open} dispatch={dispatch} />;
    case "blocked":
      return (
        <div className="row-body">
          {item.text !== RUNNER_LOST_TWICE && <span className="muted">{item.text}</span>}
          <div className="btns">
            {item.allowed_actions.includes("unblock") && (
              <button className="btn primary" onClick={() => dispatch({ type: "ticketAction", key: item.ticket, action: "unblock" })}>Unblock</button>
            )}
            {item.link && <a className="btn" href={item.link} target="_blank" rel="noreferrer" data-testid="pr-link">Open the pull request</a>}
            {open}
          </div>
        </div>
      );
    case "review": return <Review item={item} open={open} dispatch={dispatch} detail={detail} />;
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
    case "design_question": return <DesignQuestion item={item} project={item.project ?? (route?.name === "card" ? route.project : undefined)} open={open} />;
    case "pin": return null;
  }
}

/// A pin of the assistant on a design: its question, the box that answers it in place, and Open
/// card (the mockup with the pin). The row goes when the inbox is read again after the answer.
function DesignQuestion({ item, project, open }: { item: InboxItem; project: string | undefined; open: React.ReactNode | undefined }) {
  const [text, setText] = useState("");
  const [error, setError] = useState<string | undefined>();
  const send = async () => {
    const problem = answerProblem(text);
    if (problem) return setError(problem);
    try {
      await post(`/projects/${encodeURIComponent(project ?? "")}/design-pins/${item.pin}/answer`, { text: text.trim() });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  return (
    <div className="row-body" data-testid="design-question">
      <blockquote data-testid="question">{item.text}</blockquote>
      <form className="compose" onSubmit={(e) => { e.preventDefault(); void send(); }}>
        <input
          type="text" data-testid="inbox-pin-input" aria-label={`Answer to ${item.text}`} placeholder="Your answer"
          maxLength={MAX_ANSWER} value={text} onChange={(e) => setText(e.target.value)}
        />
        <button className="btn primary" type="submit" data-testid="inbox-pin-send" disabled={!text.trim() || !project}>Answer</button>
        {open}
      </form>
      {error && <span className="field-error" role="alert">{error}</span>}
    </div>
  );
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
  const [why, setWhy] = useState("");
  if (item.allowed_actions.includes("merge_now")) {
    // A pull request without checks: the card's own answers, not an agent's.
    const act = (action: "merge_now" | "keep_waiting") => dispatch({ type: "ticketAction", key: item.ticket, action });
    return (
      <div className="row-body" data-testid="no-checks">
        <blockquote data-testid="question">{item.text}</blockquote>
        <div className="btns">
          <button className="btn primary" data-testid="no-checks-merge-now" onClick={() => act("merge_now")}>Merge now</button>
          <button className="btn" data-testid="no-checks-keep-waiting" onClick={() => act("keep_waiting")}>Keep waiting</button>
          {open}
        </div>
        <form
          className="compose"
          onSubmit={(e) => {
            e.preventDefault();
            if (why.trim()) dispatch({ type: "ticketAction", key: item.ticket, action: "block", text: why.trim() });
          }}
        >
          <input aria-label={`Why ${item.ticket} is blocked`} placeholder="Why block it?" value={why} onChange={(e) => setWhy(e.target.value)} />
          <button className="btn danger" type="submit" data-testid="no-checks-block" disabled={!why.trim()}>Block</button>
        </form>
      </div>
    );
  }
  return (
    <div className="row-body">
      <Answer item={item} dispatch={dispatch} />
      <div className="btns">{open}</div>
    </div>
  );
}

/// A card for the person to review: Accept, Return (with what to change), and, when the reviewer
/// gave no verdict, the end of its screen and Run the reviewer again. The same actions as the card.
function Review({ item, open, dispatch, detail }: BodyProps & { detail: TicketDetail | undefined }) {
  const report = finishReport(detail?.comments ?? []);
  const [returning, setReturning] = useState(false);
  const [comment, setComment] = useState("");
  const act = (action: "accept" | "run_reviewer") => dispatch({ type: "ticketAction", key: item.ticket, action });
  const noVerdict = item.text === NO_VERDICT;
  const can = (a: InboxItem["allowed_actions"][number]) => item.allowed_actions.includes(a);
  return (
    <div className="row-body" data-testid="no-verdict" data-no-verdict={noVerdict}>
      {(noVerdict || item.text) && <blockquote data-testid="question">{item.text}</blockquote>}
      {item.lines && item.lines.length > 0 && <pre className="screen" data-testid="screen">{item.lines.join("\n")}</pre>}
      {item.hand_in ? <InboxHandInBlocks handIn={item.hand_in} /> : report && <HandIn report={report} />}
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
        {item.done_cards.join(", ")} {item.done_cards.length === 1 ? "was" : "were"} done before this story; these tasks are still open:
      </p>
      <ul className="ck">{item.subtasks.map((s, i) => <li key={i}>{s}</li>)}</ul>
      <div className="btns">
        <button className="btn primary" onClick={() => decide("move")}>Move to a new story</button>
        <button className="btn" onClick={() => decide("close")}>Close them</button>
        <button className="btn" onClick={() => decide("leave")}>Leave as is</button>
        {open}
      </div>
    </div>
  );
}
