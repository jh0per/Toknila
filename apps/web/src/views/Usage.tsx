import type { UsageFigures } from "../api/types";
import { ESTIMATE_NOTE, costText, shareText, tokensText } from "../format";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { type Metric, WINDOWS } from "../store/usage";
import { UsageChart } from "./UsageChart";
import { UsageQuota } from "./UsageQuota";
import { BreakdownTable } from "./UsageTables";

function Stat({ id, label, value, hint }: { id: string; label: string; value: string; hint?: string }) {
  return (
    <div className="stat" data-testid={`usage-${id}`} title={hint}>
      <span className="muted">{label}</span>
      <b>{value}</b>
    </div>
  );
}

function Totals({ t }: { t: UsageFigures }) {
  return (
    <div className="usage-totals">
      <Stat id="processed" label="Tokens processed" value={tokensText(t.processed)} />
      <Stat id="cached" label="Cached share" value={shareText(t.cached_share)} hint="Input served from the cache" />
      <Stat id="uncached" label="Uncached input" value={tokensText(t.tokens.input)} hint="Input that was neither read from the cache nor written to it" />
      <Stat id="cache-writes" label="Cache writes" value={tokensText(t.tokens.cache_write)} hint="Input written to the cache; it counted as uncached input before" />
      <Stat id="output" label="Output" value={tokensText(t.tokens.output)} />
      <Stat id="cost" label="Est. cost" value={costText(t.cost_usd)} hint={ESTIMATE_NOTE} />
      <Stat id="savings" label="Est. cache savings" value={costText(t.cache_savings_usd)} hint={ESTIMATE_NOTE} />
    </div>
  );
}

const Toggle = <T extends string | number>({ value, options, label, onPick, testId }: {
  value: T;
  options: { value: T; text: string }[];
  label: string;
  onPick: (v: T) => void;
  testId: string;
}) => (
  <div className="seg" role="group" aria-label={label} data-testid={testId}>
    {options.map((o) => (
      <button key={String(o.value)} type="button" className={`btn${o.value === value ? " on" : ""}`}
        aria-pressed={o.value === value} onClick={() => onPick(o.value)}>
        {o.text}
      </button>
    ))}
  </div>
);

/// Tokens and estimated cost: totals, a per-day chart and breakdowns. Numbers only.
export function Usage({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const { view, days, metric, error } = state.usage;
  const s = view?.summary;
  return (
    <div className="usage" data-testid="usage">
      <header className="usage-head">
        <h2>Usage</h2>
        <Toggle testId="usage-days-switch" label="Window" value={days} onPick={(d) => dispatch({ type: "setUsageDays", days: d })}
          options={WINDOWS.map((d) => ({ value: d, text: `${d}d` }))} />
        <Toggle<Metric> testId="usage-metric" label="Show" value={metric} onPick={(m) => dispatch({ type: "setUsageMetric", metric: m })}
          options={[{ value: "cost", text: "Cost" }, { value: "tokens", text: "Tokens" }]} />
      </header>
      <p className="muted" data-testid="usage-estimate">{ESTIMATE_NOTE}</p>
      {error && <p className="error">{error}</p>}
      {!view || !s ? <p className="muted">Loading…</p> : (
        <>
          <Totals t={s.totals} />
          {s.unpriced_models.length > 0 && (
            <p className="muted">No price for {s.unpriced_models.join(", ")}: counted in tokens, not in cost.</p>
          )}
          <UsageChart series={s.series} metric={metric} />
          <label className="usage-scan">
            <input type="checkbox" checked={view.machine_scan} data-testid="usage-machine-scan"
              onChange={(e) => dispatch({ type: "setMachineScan", on: e.target.checked })} />
            Count every Claude Code session on this Mac, not only Toknila runs (local only)
          </label>
          <UsageQuota quota={view.quota} />
          <div className="usage-tables">
            <BreakdownTable id="model" title="Model" items={s.by_model} metric={metric} />
            <BreakdownTable id="workspace" title="Workspace" items={s.by_workspace} metric={metric} />
            <BreakdownTable id="project" title="Project" note="Toknila runs only" items={s.by_project} metric={metric} />
            <BreakdownTable id="chain" title="Epic" note="Toknila runs only" items={s.by_chain} metric={metric} />
            <BreakdownTable id="card" title="Story" note="Toknila runs only" items={s.by_card} metric={metric} />
          </div>
          <p className="muted" data-testid="usage-prices">
            Prices: {view.prices.source}, {view.prices.retrieved}
            {view.prices.overridden.length > 0 && `; overridden: ${view.prices.overridden.join(", ")}`}
            {view.prices.problem && <span className="error"> {view.prices.problem}</span>}
          </p>
        </>
      )}
    </div>
  );
}
