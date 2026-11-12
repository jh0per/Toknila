import type { UsageItem } from "../api/types";
import { costText, shareText, tokensText } from "../format";
import { type Metric, breakdownRows } from "../store/usage";
import { workspaceRows } from "../store/usageGroups";

const Head = ({ title }: { title: string }) => (
  <thead>
    <tr><th>{title}</th><th aria-hidden="true" /><th className="num">Tokens</th><th className="num">Cached</th><th className="num">Est. cost</th></tr>
  </thead>
);

const Meter = ({ share }: { share: number }) => (
  <td className="meter"><span style={{ width: `${Math.round(share * 100)}%` }} /></td>
);

/// One breakdown: label, a bar for the metric shown, tokens, cached share and estimated cost.
/// The workspace one groups worktrees under their repository (a short name, the path on hover).
export function BreakdownTable({ id, title, note, items, metric }: {
  id: string;
  title: string;
  note?: string;
  items: UsageItem[];
  metric: Metric;
}) {
  const rows = breakdownRows(items, metric);
  return (
    <section className="usage-breakdown" data-testid={`usage-by-${id}`}>
      <h3>{title}{note && <span className="muted"> {note}</span>}</h3>
      {rows.length === 0 ? <p className="muted">Nothing in this window.</p> : (
        <table>
          <Head title={title} />
          <tbody>
            {id === "workspace" ? workspaceRows(items, metric).map((r) => (
              <tr
                key={`${r.kind}:${r.label}`} className={r.kind === "group" ? "group" : r.inGroup ? "child" : undefined}
                {...(r.kind === "group" ? { "data-group": r.label } : { "data-label": r.label })}
              >
                <td className="label" title={r.label}>{r.short}</td>
                <Meter share={r.share} />
                <td className="num">{tokensText(r.processed)}</td>
                <td className="num">{shareText(r.cachedShare)}</td>
                <td className="num">{costText(r.cost)}</td>
              </tr>
            )) : rows.map((r) => (
              <tr key={r.label} data-label={r.label}>
                <td className="label" title={r.label}>{r.label}</td>
                <Meter share={r.share} />
                <td className="num">{tokensText(r.item.figures.processed)}</td>
                <td className="num">{shareText(r.item.figures.cached_share)}</td>
                <td className="num">{costText(r.item.figures.cost_usd)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
