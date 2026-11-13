import { useEffect, useRef, useState } from "react";
import type { Ticket, TicketDetail } from "../api/types";
import { ASK_INSTEAD, type ChatItem, type Mode, canTalk, chatItems, composerPlaceholder, defaultMode, pendingText, whenText } from "../store/chat";
import type { Intent } from "../store/msg";
import { type State, currentProject, ticketEvents } from "../store/state";
import { Answer } from "./Answer";
import { PromptField } from "./AskBox";
import { MicButton, SpeechNote } from "./MicButton";
import { useSpeech } from "./useSpeech";

const KIND_LABEL: Record<ChatItem["kind"], string> = {
  instruction: "instruction", report: "report", decision: "decision", question: "question", answer: "answer", note: "", ask: "question",
};

/// The pending sentence of an instruction; on a card with no live agent its last words are a link
/// that sends the same text as a question instead.
function PendingNote({ item, card, pending, onAsk }: { item: ChatItem; card: Ticket; pending: TicketDetail["pending"]; onAsk: (text: string) => void }) {
  const text = pendingText(pending ?? undefined, card);
  const at = text.indexOf(ASK_INSTEAD);
  if (at < 0) return <span className="muted" data-testid="bubble-pending">{text}</span>;
  return (
    <span className="muted" data-testid="bubble-pending">
      {text.slice(0, at)}
      <button type="button" className="ask-instead" data-testid="ask-instead" onClick={() => onAsk(item.text)}>{ASK_INSTEAD}</button>
      {text.slice(at + ASK_INSTEAD.length)}
    </span>
  );
}

function Bubble({ item, card, pending, onAsk }: { item: ChatItem; card: Ticket; pending: TicketDetail["pending"]; onAsk: (text: string) => void }) {
  return (
    <li className={`bubble ${item.side} k-${item.kind}`} data-testid="bubble" data-kind={item.kind} data-side={item.side}>
      <div className="bubble-head">
        <b>{item.who}</b>
        {KIND_LABEL[item.kind] && <span className="kind">{KIND_LABEL[item.kind]}</span>}
        <time className="muted" dateTime={new Date(item.at).toISOString()}>{whenText(item.at)}</time>
      </div>
      <p className="gist">{item.text}</p>
      {item.more.map((line, i) => <p key={i} className={i === 0 ? "" : "muted"}>{line}</p>)}
      {item.pending && <PendingNote item={item} card={card} pending={pending} onAsk={onAsk} />}
      {item.delivered && <span className="muted" data-testid="bubble-delivered">delivered</span>}
    </li>
  );
}

/// A card's Chat: what was said between the person and the card's agent, one bubble each (its
/// comments, its questions, the answers), oldest first. When the card waits on a question, the
/// answer box is right here, the same one the Inbox has; otherwise a box sends the agent an
/// instruction.
export function ChatTab({ state, card, dispatch }: { state: State; card: Ticket; dispatch: (i: Intent) => void }) {
  const detail = state.details[card.key];
  const asked = card.status === "waiting" ? state.inbox.find((i) => i.ticket === card.key && i.kind === "question") : undefined;
  const items = chatItems(detail, ticketEvents(state, card.id), Boolean(asked));
  const [text, setText] = useState("");
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => {
    end.current?.scrollIntoView?.({ block: "nearest" });
  }, [items.length, asked?.since]);
  const project = currentProject(state)?.key;
  const mode = canTalk(card) ? defaultMode(card) : "ask";
  // Ask goes to the project's assistant with the story's record; Tell the agent is an instruction.
  const ask = (said: string) => {
    if (project && said.trim()) dispatch({ type: "askStory", project, key: card.key, text: said.trim() });
  };
  const send = (how: Mode = mode) => {
    if (!text.trim()) return;
    if (how === "ask") ask(text);
    else dispatch({ type: "addComment", key: card.key, kind: "instruction", text: text.trim() });
    setText("");
  };
  const speech = useSpeech(text, setText, () => send());
  return (
    <div className="chat" data-testid="chat">
      {!detail ? <p className="muted">Loading…</p> : items.length === 0 && !asked ? (
        <p className="muted" data-testid="chat-empty">Nothing said yet. What you and the story's agent tell each other appears here.</p>
      ) : (
        <ol className="bubbles" data-testid="chat-items">{items.map((i) => <Bubble key={i.key} item={i} card={card} pending={detail.pending ?? undefined} onAsk={ask} />)}</ol>
      )}
      {asked && (
        <div className="bubble agent open-question" data-testid="chat-answer">
          <div className="bubble-head"><b>{asked.role === "reviewer" ? "Reviewer" : "Agent"}</b><span className="kind">waiting for you</span></div>
          <Answer item={asked} dispatch={dispatch} />
        </div>
      )}
      {!asked && (
        <form className="chat-box" data-testid="chat-box" data-mode={mode} onSubmit={(e) => { e.preventDefault(); send(); }}>
          <PromptField
            value={text} onChange={(t) => { speech.stop(); setText(t); }} onSubmit={() => send()} rows={2} label="Message about this story" testId="chat-input"
            placeholder={composerPlaceholder(card, mode)}
          />
          <MicButton s={speech} testId="chat" disabled={false} />
          <button type="button" className={`btn${mode === "ask" ? " primary" : ""}`} data-testid="chat-ask" disabled={!text.trim() || !project} onClick={() => send("ask")}>Ask</button>
          {canTalk(card) && (
            <button type="button" className={`btn${mode === "tell" ? " primary" : ""}`} data-testid="chat-send" disabled={!text.trim()} onClick={() => send("tell")}>Tell the agent</button>
          )}
          <SpeechNote s={speech} testId="chat" />
        </form>
      )}
      {!asked && !canTalk(card) && <p className="muted" data-testid="chat-closed">This story is finished, so there is nobody to tell; you can still ask about it.</p>}
      <div ref={end} />
    </div>
  );
}
