import { useEffect, useRef, useState } from "react";
import type { Ticket } from "../api/types";
import { type ChatItem, canTalk, chatItems, whenText } from "../store/chat";
import type { Intent } from "../store/msg";
import { type State, ticketEvents } from "../store/state";
import { Answer } from "./Answer";
import { PromptField } from "./AskBox";

const KIND_LABEL: Record<ChatItem["kind"], string> = {
  instruction: "instruction", report: "report", decision: "decision", question: "question", answer: "answer", note: "",
};

function Bubble({ item }: { item: ChatItem }) {
  return (
    <li className={`bubble ${item.side} k-${item.kind}`} data-testid="bubble" data-kind={item.kind} data-side={item.side}>
      <div className="bubble-head">
        <b>{item.who}</b>
        {KIND_LABEL[item.kind] && <span className="kind">{KIND_LABEL[item.kind]}</span>}
        <time className="muted" dateTime={new Date(item.at).toISOString()}>{whenText(item.at)}</time>
      </div>
      <p className="gist">{item.text}</p>
      {item.more.map((line, i) => <p key={i} className={i === 0 ? "" : "muted"}>{line}</p>)}
      {item.pending && <span className="muted" data-testid="bubble-pending">not delivered yet</span>}
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
  const send = () => {
    if (!text.trim()) return;
    dispatch({ type: "addComment", key: card.key, kind: "instruction", text: text.trim() });
    setText("");
  };
  const started = card.status === "running" || card.status === "waiting" || card.status === "review";
  return (
    <div className="chat" data-testid="chat">
      {!detail ? <p className="muted">Loading…</p> : items.length === 0 && !asked ? (
        <p className="muted" data-testid="chat-empty">Nothing said yet. What you and the story's agent tell each other appears here.</p>
      ) : (
        <ol className="bubbles" data-testid="chat-items">{items.map((i) => <Bubble key={i.key} item={i} />)}</ol>
      )}
      {asked && (
        <div className="bubble agent open-question" data-testid="chat-answer">
          <div className="bubble-head"><b>{asked.role === "reviewer" ? "Reviewer" : "Agent"}</b><span className="kind">waiting for you</span></div>
          <Answer item={asked} dispatch={dispatch} />
        </div>
      )}
      {!asked && canTalk(card) && (
        <form className="chat-box" data-testid="chat-box" onSubmit={(e) => { e.preventDefault(); send(); }}>
          <PromptField
            value={text} onChange={setText} onSubmit={send} rows={2} label="Message to the agent" testId="chat-input"
            placeholder={started ? "Tell the agent something; Enter sends" : "Leave a note for the agent; it gets it when it starts"}
          />
          <button type="submit" className="btn primary" data-testid="chat-send" disabled={!text.trim()}>Send to the agent</button>
        </form>
      )}
      {!asked && !canTalk(card) && <p className="muted" data-testid="chat-closed">This story is finished, so there is nobody to tell.</p>}
      <div ref={end} />
    </div>
  );
}
