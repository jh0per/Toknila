import { useEffect, useState } from "react";
import type { Ticket } from "../api/types";
import { awaitingAnswer, discussionOf, draftKey, entries, overdue, unansweredMessage, wakeWhenOverdue } from "../store/discussion";
import type { Intent } from "../store/msg";
import { draftOf, plannerOf } from "../store/planner";
import type { State } from "../store/state";
import { canTalk } from "../store/chat";
import { canSend } from "../store/askAttach";
import { AskChips, AttachButton, DropFrame, useAskAttach } from "./AskAttach";
import { PromptField } from "./AskBox";
import { DiffView } from "./DiffView";

const OUTCOME = { accepted: "Accepted: the card has it now.", rejected: "Rejected: the card is unchanged." } as const;

/// The "Talk to the assistant" box of a card's page: what the person and the assistant said (the
/// card's discussion thread), the assistant's proposed edits as a diff with Accept and Reject, and
/// a box to say more. The box waits while an answer is pending, and when the assistant is not
/// running it offers to start it instead.
export function Discuss({ state, card, project, dispatch }: { state: State; card: Ticket; project: string; dispatch: (i: Intent) => void }) {
  useEffect(() => dispatch({ type: "refreshPlanner", project }), [project]);
  const thread = discussionOf(state.threads[card.key]);
  const items = entries(thread);
  const running = plannerOf(state, project)?.running ?? false;
  const sending = state.planner.sending === draftKey(card.key);
  const [now, setNow] = useState(() => Date.now());
  const unanswered = unansweredMessage(thread);
  useEffect(() => {
    setNow(Date.now());
    return wakeWhenOverdue(thread, Date.now(), () => setNow(Date.now()));
  }, [unanswered?.id]);
  const late = running && !sending && overdue(thread, now);
  const waiting = running && !late && (sending || awaitingAnswer(thread));
  const nudge = () =>
    unanswered && dispatch({ type: "nudgeCard", project, key: card.key, thread: thread?.thread.number ?? 1, text: unanswered.text });
  const text = draftOf(state, draftKey(card.key));
  const send = () => dispatch({ type: "discussCard", project, key: card.key });
  const decide = (id: string, action: "accept_edit" | "reject_edit") =>
    dispatch({ type: "threadAction", key: card.key, thread: String(thread?.thread.number ?? 1), action, text: id });
  const a = useAskAttach(state, dispatch, { project, where: draftKey(card.key), prefix: card.key });
  if (!canTalk(card)) return null;
  return (
    <section className="discuss" data-testid="discuss">
      <h4>Talk to the assistant</h4>
      {items.length > 0 && (
        <ol className="bubbles" data-testid="discuss-items">
          {items.map((i) =>
            i.type === "say" ? (
              <li key={i.id} className={`bubble ${i.side === "you" ? "you" : "agent"}`} data-testid="discuss-say" data-side={i.side}>
                <div className="bubble-head"><b>{i.side === "you" ? "You" : "Assistant"}</b></div>
                <p className="gist">{i.text}</p>
              </li>
            ) : (
              <li key={i.id} className="bubble agent proposal" data-testid="discuss-edit" data-state={i.state}>
                <div className="bubble-head"><b>Assistant</b><span className="kind">proposed a change</span></div>
                <DiffView proposal={i.proposal} />
                {i.state === "pending" ? (
                  <div className="row">
                    <button type="button" className="btn primary" data-testid="edit-accept" onClick={() => decide(i.id, "accept_edit")}>Accept</button>
                    <button type="button" className="btn" data-testid="edit-reject" onClick={() => decide(i.id, "reject_edit")}>Reject</button>
                  </div>
                ) : (
                  <p className="muted" data-testid="edit-outcome">{OUTCOME[i.state]}</p>
                )}
              </li>
            ),
          )}
        </ol>
      )}
      {waiting && <p className="muted" data-testid="discuss-waiting">The assistant is answering…</p>}
      {late && (
        <div className="row" data-testid="discuss-late">
          <p className="muted">The assistant has not answered yet.</p>
          <button type="button" className="btn" data-testid="discuss-nudge" onClick={nudge}>Nudge</button>
        </div>
      )}
      {state.planner.error &&<p className="error" role="alert" data-testid="discuss-error">{state.planner.error}</p>}
      {running ? (
        <DropFrame a={a} className="discuss-drop">
          <form className="chat-box" data-testid="discuss-box" onSubmit={(e) => { e.preventDefault(); if (!waiting) send(); }}>
            <PromptField
              value={text} rows={2} label="Message to the assistant" testId="discuss-input" disabled={waiting}
              placeholder="Ask about this card or say what to change; Enter sends"
              onChange={(t) => dispatch({ type: "editPlannerDraft", where: draftKey(card.key), text: t })} onSubmit={() => { if (!waiting) send(); }}
            />
            <AttachButton busy={waiting || a.place.busy} onFiles={a.add} testId="discuss" />
            <button type="submit" className="btn primary" data-testid="discuss-send" disabled={waiting || !canSend(state, draftKey(card.key), text)}>Send</button>
          </form>
          <AskChips a={a} project={project} testId="discuss" />
        </DropFrame>
      ) : (
        <div className="chat-box" data-testid="discuss-off">
          <p className="muted">The assistant is not running, so there is nobody to talk to.</p>
          <button type="button" className="btn primary" data-testid="discuss-start" onClick={() => dispatch({ type: "startPlanner", project })}>
            Start the assistant
          </button>
        </div>
      )}
    </section>
  );
}
