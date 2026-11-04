import type { Release, Ticket } from "../api/types";
import { duration, reasonText } from "../format";

function Status({ t }: { t: Ticket }) {
  return <span className={`status s-${t.status}`} data-testid="status">{t.status}</span>;
}

export function TicketRow({ t, now, human }: { t: Ticket; now: number; human: boolean }) {
  return (
    <li className={`node ticket${human ? " human" : ""}`} data-key={t.key}>
      <span className="key">{t.key}</span>
      <span className="title">{t.title}</span>
      <Status t={t} />
      {t.paused && <span className="tag">paused</span>}
      {human && <span className="tag human-tag">needs a human</span>}
      <span className="reason">{reasonText(t.status_reason)}</span>
      <span className="since" title="time in status">{duration(now - t.status_since)}</span>
    </li>
  );
}

export function ReleaseRow({ r }: { r: Release }) {
  return (
    <li className="node release" data-version={r.version}>
      <span className="key">◆ {r.version}</span>
      <span className="title">{r.channel} · {r.start_mode}</span>
      <span className={`status r-${r.state}`}>{r.state}</span>
    </li>
  );
}
