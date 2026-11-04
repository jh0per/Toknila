import type { ChainDetail } from "../api/types";
import { duration, reasonText } from "../format";
import type { Route } from "../store/route";
import { routePath } from "../store/route";
import { ReleaseRow, TicketRow } from "./Rows";

export function ChainCard({ c, now, needsHuman, onlyHuman, open }: {
  c: ChainDetail;
  now: number;
  needsHuman: Record<string, true>;
  onlyHuman: boolean;
  open: (route: Route) => void;
}) {
  const s = c.summary;
  const tickets = new Map(c.tickets.map((t) => [t.id, t]));
  const releases = new Map(c.releases.map((r) => [r.id, r]));
  const rows = c.order.flatMap((n) => {
    if (n.kind === "ticket") {
      const t = tickets.get(n.id);
      if (!t || (onlyHuman && !needsHuman[t.id])) return [];
      return [<TicketRow key={t.id} t={t} now={now} human={!!needsHuman[t.id]} />];
    }
    const r = releases.get(n.id);
    return r && !onlyHuman ? [<ReleaseRow key={r.id} r={r} />] : [];
  });
  const route: Route = { name: "chain", chain: c.chain.name };
  return (
    <section className="chain" data-chain={c.chain.name}>
      <header>
        <h2>
          <a
            href={`${routePath(route)}${window.location.search}`}
            onClick={(e) => {
              e.preventDefault();
              open(route);
            }}
          >
            {c.chain.name}
          </a>
        </h2>
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
