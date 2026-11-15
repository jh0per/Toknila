import { useEffect, useLayoutEffect, useRef } from "react";
import type { Project } from "../api/types";
import { ROADMAP_STARTER, type Context, aboutThis, contextKey, contextLabel } from "../store/assistant";
import { openTurn, talkOf } from "../store/conversation";
import type { Intent } from "../store/msg";
import { plannerOf } from "../store/planner";
import { nudgeFor } from "../store/plannerTurn";
import type { State } from "../store/state";
import { titleByName } from "../store/title";
import { AskBox } from "./AskBox";
import { CardProposals } from "./CardProposals";
import { ChatEntry } from "./ChatMessage";
import { usePlannerShown, useTurn } from "./useTurn";
import { StoppedLine, WaitingLine } from "./WaitingLine";

const placeholderFor = (c: Context | undefined): string =>
  c?.kind === "card" ? "Ask about this story or say what to change; Enter sends"
  : c?.kind === "chain" ? "Ask, or say what to change in this epic; Enter sends"
  : c?.kind === "roadmap" ? "Ask, or say what to build; Enter sends"
  : "Tell the assistant what to change; Enter sends";

/// How close to the top (in pixels) the scroll must be to ask for older messages, and how close
/// to the bottom it must be for a new message to keep it there.
const TOP_PX = 40;
const BOTTOM_PX = 80;

/// The chat of the assistant's screen: the project's conversation oldest first with the newest at
/// the bottom, what the assistant did as lines between the messages, and the box under it. The last
/// 50 are loaded; scrolling to the top loads older ones. While the assistant is answering a waiting
/// line counts the minutes and shows the step the daemon reports and what the assistant said it is
/// doing; a message sent meanwhile is kept and read with the next turn. A turn that ends without an
/// answer is said at once, with Nudge; a message no turn started for gets Nudge after 90 seconds
/// (`plannerTurn.ts`).
///
/// This is the one chat of the assistant: the panel beside the work and the assistant's own screen
/// draw it. `context` is what is open (the chip above the box, which the person can take off); the
/// screen passes none.
export function ChatPane({ project, state, dispatch, context }: { project: Project; state: State; dispatch: (i: Intent) => void; context?: Context }) {
  const key = project.key;
  useEffect(() => {
    dispatch({ type: "loadConversation", project: key });
  }, [key]);
  usePlannerShown(key, dispatch);
  const talk = talkOf(state, key);
  const messages = talk?.messages ?? [];
  const shown = state.assistant.only && context ? aboutThis(messages, context) : messages;
  const label = context && contextLabel(context, context.kind === "chain" ? titleByName(state, context.name) : undefined);
  const running = plannerOf(state, key)?.running ?? false;

  const view = useTurn(state, key);
  const { waiting, late, stopped, owed } = view;
  // The waiting line counts minutes from the message that started the turn; a message sent while
  // it is answered waits for the next turn.
  const turn = openTurn(messages);
  const queued = new Set(running ? turn.queued : []);
  // The latest progress line shows under the waiting line, so the log leaves it out meanwhile.
  const inLog = waiting && turn.progress ? shown.filter((m) => m.id !== turn.progress?.id) : shown;
  const nudge = () => owed && dispatch(nudgeFor(key, owed));

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
  }, [messages, waiting, late, stopped]);
  const onScroll = () => {
    const el = log.current;
    if (!el) return;
    seen.current.stick = el.scrollHeight - el.scrollTop - el.clientHeight < BOTTOM_PX;
    if (el.scrollTop < TOP_PX && talk?.more && !talk.older) dispatch({ type: "loadOlder", project: key });
  };

  const only = state.assistant.only && context !== undefined;
  // An empty chat on the roadmap carries the starter, and the box takes the focus.
  const starter = talk !== undefined && messages.length === 0 && context?.kind === "roadmap";
  return (
    <div className="chat-pane" data-testid="chat-pane">
      <label className="only-about muted" title={context ? undefined : "Open a roadmap, an epic or a story to filter by it"}>
        <input
          type="checkbox" data-testid="only-about" checked={only} disabled={!context}
          onChange={(e) => dispatch({ type: "setOnlyAboutThis", only: e.target.checked })}
        />{" "}
        Only about this
      </label>
      <div className="chat-log" ref={log} onScroll={onScroll} data-testid="chat-log" tabIndex={0} aria-label="Conversation with the assistant">
        {talk?.older && <p className="muted chat-note" data-testid="chat-older">Loading older messages…</p>}
        {talk && !talk.more && messages.length > 0 && <p className="muted chat-note" data-testid="chat-start">This is the start of the conversation.</p>}
        {talk && only && shown.length === 0 && messages.length > 0 && (
          <p className="muted chat-note" data-testid="chat-none-about">Nothing said about {label?.toLowerCase()} yet.</p>
        )}
        {talk && messages.length === 0 && (
          <p className="muted chat-note" data-testid="placeholder">
            {starter
              ? ROADMAP_STARTER
              : "Tell the assistant what you want to build or change. It reads your project and writes the plan as stories; you start the stories and accept the work."}
          </p>
        )}
        <ol className="chat-items" data-testid="chat-items">
          {inLog.map((m) => <ChatEntry key={m.id} m={m} project={key} state={state} dispatch={dispatch} queued={queued.has(m.id)} />)}
        </ol>
        {context?.kind === "card" && <CardProposals state={state} card={context.key} dispatch={dispatch} />}
        {waiting && <WaitingLine project={key} since={view.since} now={view.now} step={view.step} progress={turn.progress} dispatch={dispatch} />}
        {stopped && <StoppedLine onNudge={nudge} />}
        {late && !turn.progress && (
          <div className="row chat-note" data-testid="chat-late">
            <p className="muted">Nudge sends your message to the assistant again.</p>
            <button type="button" className="btn" data-testid="chat-nudge" onClick={nudge}>Nudge</button>
          </div>
        )}
      </div>
      {!running && (
        <p className="muted chat-note chat-idle" data-testid="planner-idle">
          The assistant is not running. Sending a message starts it.
        </p>
      )}
      {context && (
        <div className="context-chip-row">
          <span className="context-chip" data-testid="context-chip" data-context={contextKey(context)}>
            {label}
            <button type="button" className="icon-btn" data-testid="context-chip-remove" aria-label={`Take off the context: ${label}`} onClick={() => dispatch({ type: "removeChip" })}>
              ×
            </button>
          </span>
        </div>
      )}
      <AskBox
        project={key} state={state} dispatch={dispatch} testId="planner-ask" compact focus={starter}
        label="Message to the assistant" placeholder={placeholderFor(context)} button="Send"
      />
    </div>
  );
}
