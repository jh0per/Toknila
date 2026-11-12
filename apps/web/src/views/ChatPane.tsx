import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { Project } from "../api/types";
import { talkOf, unanswered, wakeWhenLate } from "../store/conversation";
import { msUntilOverdue } from "../store/conversation";
import type { Intent } from "../store/msg";
import { plannerOf } from "../store/planner";
import type { State } from "../store/state";
import { AskBox } from "./AskBox";
import { ChatEntry } from "./ChatMessage";

/// How close to the top (in pixels) the scroll must be to ask for older messages, and how close
/// to the bottom it must be for a new message to keep it there.
const TOP_PX = 40;
const BOTTOM_PX = 80;

/// The chat of the assistant's screen: the project's conversation oldest first with the newest at
/// the bottom, what the assistant did as lines between the messages, and the box under it. The last
/// 50 are loaded; scrolling to the top loads older ones. While the assistant is answering the box
/// waits, and after 90 seconds it says so and offers a nudge.
export function ChatPane({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const key = project.key;
  useEffect(() => {
    dispatch({ type: "loadConversation", project: key });
    dispatch({ type: "refreshPlanner", project: key });
  }, [key]);
  const talk = talkOf(state, key);
  const messages = talk?.messages ?? [];
  const running = plannerOf(state, key)?.running ?? false;
  const sending = state.planner.sending === key;

  const [now, setNow] = useState(() => Date.now());
  const owed = unanswered(messages);
  useEffect(() => {
    setNow(Date.now());
    return wakeWhenLate(messages, Date.now(), () => setNow(Date.now()));
  }, [owed?.id]);
  const late = running && !sending && msUntilOverdue(messages, now) === 0;
  const waiting = running && !late && (sending || owed !== undefined);
  const nudge = () => {
    if (!owed) return;
    const about = owed.about;
    if (about?.kind === "card" && owed.thread) dispatch({ type: "nudgeCard", project: key, key: about.key, thread: owed.thread, text: owed.text });
    else dispatch({ type: "nudgeChat", project: key, text: owed.text });
  };

  // The newest message stays in view; an older page joins above without moving what is read.
  const log = useRef<HTMLDivElement>(null);
  const seen = useRef<{ first?: string; last?: string; height: number; stick: boolean }>({ height: 0, stick: true });
  useLayoutEffect(() => {
    const el = log.current;
    if (!el) return;
    const [first, last] = [messages[0]?.id, messages[messages.length - 1]?.id];
    const was = seen.current;
    if (was.first !== undefined && first !== was.first && last === was.last) el.scrollTop += el.scrollHeight - was.height;
    else if (last !== was.last && (was.stick || was.last === undefined || messages[messages.length - 1]?.author === "person")) el.scrollTop = el.scrollHeight;
    seen.current = { first, last, height: el.scrollHeight, stick: was.stick };
  }, [messages, waiting, late]);
  const onScroll = () => {
    const el = log.current;
    if (!el) return;
    seen.current.stick = el.scrollHeight - el.scrollTop - el.clientHeight < BOTTOM_PX;
    if (el.scrollTop < TOP_PX && talk?.more && !talk.older) dispatch({ type: "loadOlder", project: key });
  };

  return (
    <div className="chat-pane" data-testid="chat-pane">
      <div className="chat-log" ref={log} onScroll={onScroll} data-testid="chat-log" tabIndex={0} aria-label="Conversation with the assistant">
        {talk?.older && <p className="muted chat-note" data-testid="chat-older">Loading older messages…</p>}
        {talk && !talk.more && messages.length > 0 && <p className="muted chat-note" data-testid="chat-start">This is the start of the conversation.</p>}
        {talk && messages.length === 0 && (
          <p className="muted chat-note" data-testid="placeholder">
            Tell the assistant what you want to build or change. It reads your project and writes the plan as cards; you start the cards and accept the work.
          </p>
        )}
        <ol className="chat-items" data-testid="chat-items">
          {messages.map((m) => <ChatEntry key={m.id} m={m} project={key} state={state} dispatch={dispatch} />)}
        </ol>
        {waiting && <p className="muted chat-note" data-testid="chat-waiting">The assistant is answering…</p>}
        {late && (
          <div className="row chat-note" data-testid="chat-late">
            <p className="muted">The assistant has not answered yet.</p>
            <button type="button" className="btn" data-testid="chat-nudge" onClick={nudge}>Nudge</button>
          </div>
        )}
      </div>
      {!running && (
        <p className="muted chat-note chat-idle" data-testid="planner-idle">
          The assistant is not running. Sending a message starts it.
        </p>
      )}
      <AskBox
        project={key} state={state} dispatch={dispatch} testId="planner-ask" compact waiting={waiting}
        label="Message to the assistant" placeholder="Tell the assistant what to change; Enter sends" button="Send"
      />
    </div>
  );
}
