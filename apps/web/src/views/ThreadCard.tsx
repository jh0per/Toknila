import { useState } from "react";
import type { ThreadDetail } from "../api/types";
import type { Intent } from "../store/msg";
import { shownVersion, threadStatus } from "../store/threads";
import { Artifact } from "./Artifact";

const STATUS_LABEL = { draft: "draft", open: "open", changes_requested: "changes requested", approved: "approved" } as const;
const TAG_CLASS = { draft: "muted", open: "open", changes_requested: "chg", approved: "ok" } as const;

/// One thread: its artifact version, version history, messages, and what a person can do.
export function ThreadCard(props: {
  ticket: string;
  detail: ThreadDetail;
  picked: number | undefined;
  dispatch: (i: Intent) => void;
}) {
  const { ticket, detail, picked, dispatch } = props;
  const { thread, messages, versions } = detail;
  const status = threadStatus(detail);
  const version = shownVersion(detail, picked);
  const [text, setText] = useState("");
  const closed = status === "approved";
  const draft = status === "draft";
  const act = (action: "message" | "changes" | "approve") => {
    dispatch({ type: "threadAction", key: ticket, thread: thread.id, action, text: text.trim() || undefined });
    setText("");
  };
  return (
    <article className={draft ? "thr draft" : "thr"} data-testid="thread" data-status={status} data-number={thread.number}>
      <header>
        <span className="kind">{thread.kind}</span>
        <b>{thread.title}</b>
        <span className="grow" />
        <span className={`tag ${TAG_CLASS[status]}`} data-testid="thread-state">{STATUS_LABEL[status]}</span>
      </header>
      {draft && (
        <p className="muted" data-testid="draft-note">No version yet</p>
      )}
      {version && (
        <div className="art">
          <Artifact version={version} />
        </div>
      )}
      {versions.length > 0 && (
        <div className="versions" data-testid="versions">
          <span className="muted">Versions</span>
          {versions.map((v) => (
            <button
              key={v.id}
              className={`btn ${v.number === version?.number ? "on" : ""}`}
              aria-pressed={v.number === version?.number}
              onClick={() => dispatch({ type: "pickVersion", thread: thread.id, number: v.number })}
            >
              v{v.number}{thread.approved_version === v.number ? " ✓" : ""}
            </button>
          ))}
        </div>
      )}
      <ol className="messages" data-testid="messages">
        {messages.map((m) => (
          <li key={m.id} className={`msg ${m.author} ${m.kind}`}>
            <span className="who">{m.author}</span>
            {m.kind === "changes_requested" && <span className="tag chg">changes requested</span>}
            <span className="text">{m.text}</span>
          </li>
        ))}
      </ol>
      <footer>
        <textarea
          aria-label={`Message in thread ${thread.number}`}
          placeholder={closed ? "Approved" : "Write a message, or what to change"}
          disabled={closed}
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
        <button className="btn" disabled={closed || !text.trim()} onClick={() => act("message")}>Post message</button>
        <button className="btn" disabled={closed || !text.trim()} onClick={() => act("changes")}>Request changes</button>
        <button className="btn primary" disabled={closed || draft} onClick={() => act("approve")}>Approve</button>
      </footer>
    </article>
  );
}
