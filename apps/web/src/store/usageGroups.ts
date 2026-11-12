// The workspace breakdown of the Usage page: worktrees under their repository. The daemon sends
// one line per working folder; the grouping is a view of those lines.

import type { UsageItem } from "../api/types";
import type { Metric } from "./usage";

/// One line of the grouped workspace table.
export interface WorkspaceRow {
  /// A repository line (sums of its worktrees) or a folder.
  kind: "group" | "folder";
  /// The full path of a folder, the repository name of a group: the row's `data-label`.
  label: string;
  /// What the row shows as its name.
  short: string;
  /// The metric shown, and its share of the largest line (the bar).
  value: number;
  share: number;
  processed: number;
  cachedShare: number;
  cost: number;
  /// A folder inside a group is indented.
  inGroup: boolean;
  /// How many folders a group holds (0 on a folder).
  count: number;
}

const shortName = (path: string): string => path.split(/[\\/]+/).filter((s) => s !== "").slice(-1)[0] ?? path;

const MARK = ".toknila-ships";

const segments = (path: string): string[] => path.split(/[\\/]+/).filter((s) => s !== "");

/// The repository a path is a ship of (`…/.toknila-ships/<repo>/<chain>/<card>`) and the short
/// name inside it (`<chain>/<card>`), or nothing for any other folder.
export function shipOf(path: string): { repo: string; short: string } | undefined {
  const parts = segments(path);
  const at = parts.lastIndexOf(MARK);
  if (at < 0 || parts.length - at < 4) return undefined;
  return { repo: parts[at + 1], short: parts.slice(at + 2).join("/") };
}

const metricOf = (i: UsageItem, metric: Metric) => (metric === "cost" ? i.figures.cost_usd : i.figures.processed);

interface Sum {
  processed: number;
  cost: number;
  read: number;
  input: number;
}

const sumOf = (items: UsageItem[]): Sum =>
  items.reduce<Sum>(
    (a, i) => ({
      processed: a.processed + i.figures.processed,
      cost: a.cost + i.figures.cost_usd,
      read: a.read + i.figures.tokens.cache_read,
      input: a.input + i.figures.uncached_input + i.figures.tokens.cache_read,
    }),
    { processed: 0, cost: 0, read: 0, input: 0 },
  );

/// Names for folders that are not ships: the last folder, plus its parent when two share it.
function folderNames(paths: string[]): Map<string, string> {
  const last = (p: string) => segments(p).slice(-1)[0] ?? p;
  const counts = new Map<string, number>();
  for (const p of paths) counts.set(last(p), (counts.get(last(p)) ?? 0) + 1);
  return new Map(paths.map((p) => [p, (counts.get(last(p)) ?? 0) > 1 ? segments(p).slice(-2).join("/") : last(p)]));
}

/// The table rows: each repository as a line with its worktrees under it, every other folder as
/// a line of its own, all ordered by the metric shown, largest first. Bars are of the largest line.
export function workspaceRows(items: UsageItem[], metric: Metric): WorkspaceRow[] {
  // Group by the repository the daemon found (`repo`, the main repository's folder). A folder
  // the daemon knows no repository for is grouped by its ship path when it is a ship of a
  // repository already shown, else it is a line of its own.
  const groups = new Map<string, { name: string; members: UsageItem[] }>();
  for (const item of items) {
    if (!item.repo) continue;
    const g = groups.get(item.repo) ?? { name: item.name ?? shortName(item.repo), members: [] };
    g.members.push(item);
    groups.set(item.repo, g);
  }
  const loose: UsageItem[] = [];
  for (const item of items) {
    if (item.repo) continue;
    const ship = shipOf(item.label);
    if (!ship) {
      loose.push(item);
      continue;
    }
    const home = [...groups.values()].find((g) => g.name === ship.repo);
    if (home) home.members.push(item);
    else groups.set(ship.repo, { name: ship.repo, members: [item] });
  }
  const names = folderNames(loose.map((i) => i.label));
  const row = (kind: WorkspaceRow["kind"], label: string, short: string, sum: Sum, value: number, inGroup: boolean, count = 0): WorkspaceRow => ({
    kind, label, short, value, share: 0, processed: sum.processed, cost: sum.cost,
    cachedShare: sum.input === 0 ? 0 : sum.read / sum.input, inGroup, count,
  });
  const entries: { value: number; rows: WorkspaceRow[] }[] = [];
  for (const [repo, { name, members }] of groups) {
    const sorted = [...members].sort((a, b) => metricOf(b, metric) - metricOf(a, metric));
    const value = members.reduce((n, i) => n + metricOf(i, metric), 0);
    const kidNames = folderNames(sorted.map((i) => i.label));
    const kids = sorted.map((i) =>
      row("folder", i.label, shipOf(i.label)?.short ?? kidNames.get(i.label) ?? i.label, sumOf([i]), metricOf(i, metric), true),
    );
    entries.push({ value, rows: [row("group", repo, name, sumOf(members), value, false, members.length), ...kids] });
  }
  for (const i of loose) {
    entries.push({ value: metricOf(i, metric), rows: [row("folder", i.label, names.get(i.label) ?? i.label, sumOf([i]), metricOf(i, metric), false)] });
  }
  entries.sort((a, b) => b.value - a.value);
  const top = entries.reduce((n, e) => Math.max(n, e.value), 0);
  return entries.flatMap((e) => e.rows).map((r) => ({ ...r, share: top === 0 ? 0 : r.value / top }));
}
