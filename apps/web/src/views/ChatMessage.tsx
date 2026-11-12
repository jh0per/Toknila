import { type ReactNode, useEffect, useRef, useState } from "react";
import type { ConversationMessage } from "../api/types";
import { aboutLabel, aboutRoute } from "../store/conversation";
import { whenText } from "../store/chat";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { writeClipboard } from "../terminal/clipboard";
import { NavLink } from "./Link";

/// A Copy button that puts `text` on the clipboard and says so for a moment.
function CopyButton({ text }: { text: string }) {
  const [done, setDone] = useState<"copied" | "refused" | undefined>();
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>();
  useEffect(() => () => clearTimeout(timer.current), []);
  const copy = () => {
    void writeClipboard(text).then((ok) => {
      setDone(ok ? "copied" : "refused");
      clearTimeout(timer.current);
      timer.current = setTimeout(() => setDone(undefined), 1500);
    });
  };
  return (
    <button type="button" className="copy" data-testid="chat-copy" aria-label="Copy this message" onClick={copy}>
      {done === "copied" ? "Copied" : done === "refused" ? "Not copied" : "Copy"}
    </button>
  );
}

/// What a message was about, as a chip that opens it when the store knows where it is.
function Chip({ m, project, state, dispatch }: { m: ConversationMessage; project: string; state: State; dispatch: (i: Intent) => void }) {
  if (!m.about) return null;
  const route = aboutRoute(state, project, m.about);
  const label = aboutLabel(m.about);
  return route ? (
    <NavLink route={route} dispatch={dispatch} className="chip" testId="chat-chip">{label}</NavLink>
  ) : (
    <span className="chip" data-testid="chat-chip">{label}</span>
  );
}

/// A line of Toknila's: what the assistant did, with its card or chain as the link. The key or
/// name inside the words is the link; a line that does not name it gets the link after the words.
function DidLine({ m, project, state, dispatch }: { m: ConversationMessage; project: string; state: State; dispatch: (i: Intent) => void }) {
  const route = aboutRoute(state, project, m.about);
  const label = m.about ? (m.about.kind === "chain" ? m.about.name : m.about.key) : "";
  const at = route && label ? m.text.indexOf(label) : -1;
  const link = (text: string): ReactNode =>
    route ? <NavLink route={route} dispatch={dispatch} testId="chat-did-link">{text}</NavLink> : text;
  return (
    <li className="did" data-testid="chat-did" data-id={m.id}>
      {at >= 0 ? (
        <span>{m.text.slice(0, at)}{link(label)}{m.text.slice(at + label.length)}</span>
      ) : (
        <span>{m.text}{route && <> · {link(label)}</>}</span>
      )}
      <time className="muted" dateTime={new Date(m.at).toISOString()}>{whenText(m.at)}</time>
    </li>
  );
}

/// One entry of the conversation: the person's and the assistant's words are plain selectable
/// text with a Copy control; what the assistant did is a line of Toknila's.
export function ChatEntry({ m, project, state, dispatch }: { m: ConversationMessage; project: string; state: State; dispatch: (i: Intent) => void }) {
  if (m.author === "toknila") return <DidLine m={m} project={project} state={state} dispatch={dispatch} />;
  const mine = m.author === "person";
  return (
    <li className={`bubble msg ${mine ? "you" : "agent"}`} data-testid="chat-message" data-author={m.author} data-id={m.id}>
      <div className="bubble-head">
        <b>{mine ? "You" : "Assistant"}</b>
        <Chip m={m} project={project} state={state} dispatch={dispatch} />
        <time className="muted" dateTime={new Date(m.at).toISOString()}>{whenText(m.at)}</time>
        <CopyButton text={m.text} />
      </div>
      <p className="gist" data-testid="chat-text">{m.text}</p>
      {(m.attachments ?? []).length > 0 && (
        <div className="chips" data-testid="chat-attachments">
          {(m.attachments ?? []).map((a) => <span key={a} className="chip">{a}</span>)}
        </div>
      )}
    </li>
  );
}
