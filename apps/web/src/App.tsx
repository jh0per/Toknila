import { useCallback, useEffect, useRef, useState } from "react";
import {
  type ChainDetail, type EventRecord, type Release, type Ticket, type TicketItem,
  duration, get, reasonText, streamUrl, token,
} from "./api";

type Link = "connecting" | "live" | "offline";

interface Data {
  chains: ChainDetail[];
  needsHuman: Set<string>;
}

function useNow(ms: number): number {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), ms);
    return () => clearInterval(t);
  }, [ms]);
  return now;
}

/// Loads everything, then refreshes on every journal event from the stream. Reconnects with
/// backoff and resumes after the last seq it saw, so nothing is missed across a daemon restart.
function useData(): { data: Data | null; error: string; link: Link } {
  const [data, setData] = useState<Data | null>(null);
  const [error, setError] = useState("");
  const [link, setLink] = useState<Link>("connecting");
  const lastSeq = useRef(0);
  const pending = useRef<number | null>(null);

  const load = useCallback(async () => {
    try {
      const list = await get<{ chain: { id: string } }[]>("/chains");
      const chains = await Promise.all(list.map((c) => get<ChainDetail>(`/chains/${c.chain.id}`)));
      const tickets = await get<TicketItem[]>("/tickets");
      setData({ chains, needsHuman: new Set(tickets.filter((t) => t.needs_human).map((t) => t.ticket.id)) });
      setError("");
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    }
  }, []);

  useEffect(() => {
    let ws: WebSocket | null = null;
    let retry = 500;
    let stopped = false;
    let timer: number | undefined;
    const refresh = () => {
      if (pending.current !== null) return;
      pending.current = window.setTimeout(() => {
        pending.current = null;
        void load();
      }, 50);
    };
    const connect = () => {
      setLink("connecting");
      ws = new WebSocket(streamUrl(lastSeq.current));
      ws.onopen = () => {
        retry = 500;
        setLink("live");
        void load();
      };
      ws.onmessage = (m) => {
        try {
          const e = JSON.parse(String(m.data)) as EventRecord;
          if (typeof e.seq === "number") lastSeq.current = Math.max(lastSeq.current, e.seq);
        } catch {
          /* not an event */
        }
        refresh();
      };
      ws.onclose = () => {
        if (stopped) return;
        setLink("offline");
        timer = window.setTimeout(connect, retry);
        retry = Math.min(retry * 2, 10_000);
      };
    };
    // Start from the newest event: the first load already has the state before it.
    get<EventRecord[]>("/events?since=0&limit=1000000")
      .then((evs) => {
        lastSeq.current = evs.length ? evs[evs.length - 1].seq : 0;
      })
      .catch(() => {})
      .finally(connect);
    return () => {
      stopped = true;
      clearTimeout(timer);
      ws?.close();
    };
  }, [load]);

  return { data, error, link };
}

function Status({ t }: { t: Ticket }) {
  return <span className={`status s-${t.status}`} data-testid="status">{t.status}</span>;
}

function TicketRow({ t, now, human }: { t: Ticket; now: number; human: boolean }) {
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

function ReleaseRow({ r }: { r: Release }) {
  return (
    <li className="node release" data-version={r.version}>
      <span className="key">◆ {r.version}</span>
      <span className="title">{r.channel} · {r.start_mode}</span>
      <span className={`status r-${r.state}`}>{r.state}</span>
    </li>
  );
}

function ChainCard({ c, now, needsHuman, onlyHuman }: {
  c: ChainDetail; now: number; needsHuman: Set<string>; onlyHuman: boolean;
}) {
  const s = c.summary;
  const tickets = new Map(c.tickets.map((t) => [t.id, t]));
  const releases = new Map(c.releases.map((r) => [r.id, r]));
  const rows = c.order.flatMap((n) => {
    if (n.kind === "ticket") {
      const t = tickets.get(n.id);
      if (!t || (onlyHuman && !needsHuman.has(t.id))) return [];
      return [<TicketRow key={t.id} t={t} now={now} human={needsHuman.has(t.id)} />];
    }
    const r = releases.get(n.id);
    return r && !onlyHuman ? [<ReleaseRow key={r.id} r={r} />] : [];
  });
  return (
    <section className="chain" data-chain={c.chain.name}>
      <header>
        <h2>{c.chain.name}</h2>
        <span className={`chain-status c-${s.status}`} data-testid="chain-status">{s.status.replace(/_/g, " ")}</span>
        <span className="progress">{s.done}/{s.total} done · {s.released} released</span>
      </header>
      {c.chain.goal && <p className="goal">{c.chain.goal}</p>}
      {s.stopped_at && (
        <p className="stopped">
          Stopped at <b>{s.stopped_at.ticket}</b> ({s.stopped_at.status}
          {s.stopped_at.reason ? `, ${reasonText(s.stopped_at.reason)}` : ""}) for {duration(s.stopped_at.waiting_ms)}
        </p>
      )}
      {s.next_release && <p className="next">Next milestone: {s.next_release.version} ({s.next_release.state})</p>}
      <ol className="nodes">{rows}</ol>
    </section>
  );
}

export function App() {
  const { data, error, link } = useData();
  const now = useNow(1000);
  const [onlyHuman, setOnlyHuman] = useState(false);
  if (!token()) {
    return <main><p className="error">Open the address that <code>tok open</code> prints; it carries the access token.</p></main>;
  }
  return (
    <main>
      <header className="top">
        <h1>Toknila</h1>
        <span className={`link l-${link}`} data-testid="link">{link}</span>
        <label>
          <input type="checkbox" checked={onlyHuman} onChange={(e) => setOnlyHuman(e.target.checked)} />
          needs a human
        </label>
      </header>
      {error && <p className="error">{error}</p>}
      {data === null ? <p>Loading…</p> : data.chains.length === 0 ? (
        <p className="empty">No chains yet. Try <code>tok chain import examples/demo.toml</code>.</p>
      ) : (
        data.chains.map((c) => (
          <ChainCard key={c.chain.id} c={c} now={now} needsHuman={data.needsHuman} onlyHuman={onlyHuman} />
        ))
      )}
    </main>
  );
}
