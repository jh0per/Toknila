import type { QuotaWindow, UsageView } from "../api/types";
import { agoText } from "../format";

const NAMES: Record<string, string> = { five_hour: "5-hour window", seven_day: "7-day window" };
/// The five-hour window first: it is the one that runs out.
const ORDER = ["five_hour", "seven_day"];

const resets = (w: QuotaWindow) => new Date(w.resets_at * 1000).toLocaleString();
const rank = (w: QuotaWindow) => (ORDER.includes(w.name) ? ORDER.indexOf(w.name) : ORDER.length);

/// The plan quota as Claude Code reported it, how old that figure is, or one line saying why there is none.
export function UsageQuota({ quota }: { quota: UsageView["quota"] }) {
  const seen = quota.available && quota.seen_at != null ? quota.seen_at : undefined;
  return (
    <section className="usage-quota" data-testid="usage-quota">
      <header className="quota-head">
        <h3>Remaining plan quota</h3>
        {seen !== undefined && (
          <span className="muted" data-testid="quota-age" title={new Date(seen).toLocaleString()}>
            as of {agoText(seen, Date.now())}
          </span>
        )}
      </header>
      {!quota.available ? (
        <p className="muted" data-testid="quota-unavailable">{quota.reason}</p>
      ) : (
        <>
          {[...quota.windows].sort((a, b) => rank(a) - rank(b)).map((w) => (
            <div key={w.name} className="quota-row" data-window={w.name}>
              <span>{NAMES[w.name] ?? w.name}</span>
              <span
                className="meter" role="meter" aria-valuemin={0} aria-valuemax={100}
                aria-valuenow={Math.min(100, Math.round(w.used_percentage))}
                aria-label={`${NAMES[w.name] ?? w.name} used`}
              >
                <span style={{ width: `${Math.min(100, w.used_percentage)}%` }} />
              </span>
              <span className="num">{Math.round(w.used_percentage)}% used</span>
              <span className="muted">resets {resets(w)}</span>
            </div>
          ))}
          <p className="muted">{quota.reason}</p>
        </>
      )}
    </section>
  );
}
