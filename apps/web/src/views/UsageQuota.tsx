import type { QuotaWindow, UsageView } from "../api/types";

const NAMES: Record<string, string> = { five_hour: "5-hour window", seven_day: "7-day window" };

const resets = (w: QuotaWindow) => new Date(w.resets_at * 1000).toLocaleString();

/// The plan quota as Claude Code reported it, or one line saying why there is none.
export function UsageQuota({ quota }: { quota: UsageView["quota"] }) {
  return (
    <section className="usage-quota" data-testid="usage-quota">
      <h3>Remaining plan quota</h3>
      {!quota.available ? (
        <p className="muted" data-testid="quota-unavailable">{quota.reason}</p>
      ) : (
        <>
          {quota.windows.map((w) => (
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
