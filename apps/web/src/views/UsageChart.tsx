import { useState } from "react";
import type { DayUsage } from "../api/types";
import { costText, tokensText } from "../format";
import { type Metric, maxOf } from "../store/usage";

const W = 640;
const H = 170;
const PAD = { left: 8, right: 8, top: 10, bottom: 22 };

const valueOf = (d: DayUsage, metric: Metric) => (metric === "cost" ? d.figures.cost_usd : d.figures.processed);
const text = (v: number, metric: Metric) => (metric === "cost" ? costText(v) : `${tokensText(v)} tok`);

/// One bar per UTC day, in one colour (a single series needs no legend). Hovering or focusing a
/// bar reads its day in the line above; the table below holds the same numbers.
export function UsageChart({ series, metric }: { series: DayUsage[]; metric: Metric }) {
  const [at, setAt] = useState<number | undefined>();
  const top = maxOf(series.map((d) => valueOf(d, metric)));
  const slot = (W - PAD.left - PAD.right) / Math.max(1, series.length);
  const bar = Math.max(2, Math.min(28, slot - 2));
  const base = H - PAD.bottom;
  const shown = at === undefined ? undefined : series[at];
  return (
    <div className="usage-chart">
      <p className="muted usage-readout" aria-live="polite" data-testid="usage-readout">
        {shown ? `${shown.day}: ${text(valueOf(shown, metric), metric)}` : `Per day (UTC), ${metric === "cost" ? "estimated cost" : "tokens processed"}`}
      </p>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={`Daily ${metric === "cost" ? "estimated cost" : "tokens"}, last ${series.length} days`}
        data-testid="usage-chart"
        onMouseLeave={() => setAt(undefined)}
      >
        <line className="axis" x1={PAD.left} x2={W - PAD.right} y1={base} y2={base} />
        {series.map((d, i) => {
          const v = valueOf(d, metric);
          const h = top === 0 ? 0 : Math.max(v > 0 ? 2 : 0, (v / top) * (base - PAD.top));
          const x = PAD.left + i * slot + (slot - bar) / 2;
          return (
            <g key={d.day} className={at === i ? "on" : ""} data-day={d.day}>
              {h > 0 && <rect className="bar" x={x} y={base - h} width={bar} height={h} rx={Math.min(4, bar / 2)} />}
              <rect className="hit" x={PAD.left + i * slot} y={PAD.top} width={slot} height={base - PAD.top}
                tabIndex={0} onMouseEnter={() => setAt(i)} onFocus={() => setAt(i)} onBlur={() => setAt(undefined)}>
                <title>{`${d.day}: ${text(v, metric)}`}</title>
              </rect>
            </g>
          );
        })}
        {series.length > 0 && (
          <>
            <text className="tick" x={PAD.left} y={H - 6}>{series[0].day}</text>
            <text className="tick" x={W - PAD.right} y={H - 6} textAnchor="end">{series[series.length - 1].day}</text>
          </>
        )}
      </svg>
      <details className="usage-table-view">
        <summary>Show as table</summary>
        <table data-testid="usage-days">
          <thead><tr><th>Day (UTC)</th><th>Tokens</th><th>Est. cost</th></tr></thead>
          <tbody>
            {series.map((d) => (
              <tr key={d.day}><td>{d.day}</td><td>{tokensText(d.figures.processed)}</td><td>{costText(d.figures.cost_usd)}</td></tr>
            ))}
          </tbody>
        </table>
      </details>
    </div>
  );
}
