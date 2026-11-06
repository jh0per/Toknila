import type { UsageItem } from "../api/types";
import { costText, shareText, tokensText } from "../format";
import { type Metric, breakdownRows } from "../store/usage";

/// One breakdown: label, a bar for the metric shown, tokens, cached share and estimated cost.
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
          <thead>
            <tr><th>{title}</th><th aria-hidden="true" /><th className="num">Tokens</th><th className="num">Cached</th><th className="num">Est. cost</th></tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.label} data-label={r.label}>
                <td className="label" title={r.label}>{r.label}</td>
                <td className="meter"><span style={{ width: `${Math.round(r.share * 100)}%` }} /></td>
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
