// Plans into a project from the window: the import dialog (a pasted chain file, or a path the
// daemon reads: a chain file, a spec folder, a folder of spec folders) and the plans found in a
// project's repository, picked and imported one by one. The daemon reads, judges and imports;
// this keeps its answers for the form they belong to.

import type { ChainDetail, FoundPlan, ImportPathResult, PlanScan } from "../api/types";
import type { Effect } from "./msg";
import { withChain } from "./plan";
import type { State } from "./state";

/// Which input of the import dialog an import came from.
export type ImportSource = "paste" | "path";

/// What became of one found plan's import.
export type PlanOutcome =
  | { state: "running" }
  | { state: "done"; chains: string[] }
  | { state: "failed"; message: string };

/// The plans of a project's repository while the person picks which to import.
export interface PlanList {
  /// Project key.
  project: string;
  plans: FoundPlan[];
  specify_dir: boolean;
  /// The working copy the plans are read from is behind its upstream.
  behind: PlanScan["behind"];
  /// Paths ticked for import.
  selected: string[];
  outcomes: Record<string, PlanOutcome>;
}

export interface ImportState {
  busy: boolean;
  /// The answer of the last import in the dialog.
  result: ImportPathResult | undefined;
  /// A refusal of the last import in the dialog, for the input it came from.
  error: { source: ImportSource; message: string } | undefined;
  found: PlanList | undefined;
  /// Key of the project whose repository is being scanned.
  scanning: string | undefined;
  scanError: string | undefined;
}

export const initialImports: ImportState = {
  busy: false, result: undefined, error: undefined, found: undefined, scanning: undefined, scanError: undefined,
};

/// Why the repository was scanned: right after adding the project (the list opens only when it
/// finds plans), or because the person asked (the list shows, empty or not).
export type ScanReason = "added" | "asked";

export type ImportIntent =
  | { type: "importText"; project: string; toml: string }
  | { type: "importPath"; project: string; path: string }
  | { type: "scanPlans"; project: string }
  | { type: "togglePlan"; path: string; on: boolean }
  | { type: "importSelected" }
  | { type: "skipPlans" };

export type ImportResult =
  | { type: "textImported"; project: string; chain: ChainDetail }
  | { type: "importAnswered"; project: string; result: ImportPathResult }
  | { type: "importRefused"; source: ImportSource; message: string }
  | { type: "plansScanned"; project: string; reason: ScanReason; scan: PlanScan }
  | { type: "scanFailed"; project: string; message: string }
  | { type: "planImported"; project: string; path: string; result?: ImportPathResult; message?: string }
  | { type: "plansImported"; project: string };

export type ImportMsg = ImportIntent | ImportResult;

export type ImportEffect =
  | Extract<ImportIntent, { type: "importText" | "importPath" }>
  | { type: "loadPlans"; project: string; reason: ScanReason }
  | { type: "importPlans"; project: string; paths: string[] };

/// The list a scan shows: every plan ticked except those already imported.
export function listOf(project: string, scan: PlanScan): PlanList {
  return {
    project,
    plans: scan.plans,
    specify_dir: scan.specify_dir,
    behind: scan.behind,
    selected: scan.plans.filter((p) => !p.imported).map((p) => p.path),
    outcomes: {},
  };
}

/// The answer to a pasted chain file, in the shape of an import by path.
export function pastedResult(chain: ChainDetail): ImportPathResult {
  return {
    chains: [{ folder: "pasted text", path: "", cards: chain.tickets.length, tasks: 0, done_cards: 0, out_of_order: [], chain }],
    skipped: [],
    failed: [],
  };
}

/// The name of the first chain the last "Import selected" made, in the list's order.
export function firstImported(f: PlanList): string | undefined {
  for (const p of f.plans) {
    const o = f.outcomes[p.path];
    if (o?.state === "done" && o.chains.length) return o.chains[0];
  }
  return undefined;
}

/// Whether an import of the list is still on its way.
export const importing = (f: PlanList): boolean => Object.values(f.outcomes).some((o) => o.state === "running");

/// The store with the chains an import made.
export const withImported = (s: State, r: ImportPathResult): State => r.chains.reduce((at, c) => withChain(at, c.chain), s);

/// One plan's outcome from the daemon's answer: the chains it made, or why it made none.
function outcomeOf(msg: Extract<ImportResult, { type: "planImported" }>): PlanOutcome {
  const r = msg.result;
  if (r && r.chains.length) return { state: "done", chains: r.chains.map((c) => c.chain.chain.name) };
  const why = r?.failed.map((f) => `${f.folder}: ${f.reason}`).join("; ");
  return { state: "failed", message: msg.message ?? (why || "nothing was imported") };
}

/// The reducer of imports; `reduce` adds the moves of the page (to the first imported chain).
export function reduceImports(s: State, msg: ImportMsg): [State, Effect[]] {
  const set = (imports: Partial<ImportState>, base: State = s): State => ({ ...base, imports: { ...base.imports, ...imports } });
  const f = s.imports.found;
  switch (msg.type) {
    case "importText":
    case "importPath":
      return [set({ busy: true, error: undefined, result: undefined }), [msg]];
    case "textImported":
      return [set({ busy: false, result: pastedResult(msg.chain) }, withChain(s, msg.chain)), []];
    case "importAnswered":
      return [set({ busy: false, result: msg.result }, withImported(s, msg.result)), []];
    case "importRefused":
      return [set({ busy: false, error: { source: msg.source, message: msg.message } }), []];
    case "scanPlans":
      return [set({ scanning: msg.project, scanError: undefined }), [{ type: "loadPlans", project: msg.project, reason: "asked" }]];
    case "plansScanned": {
      // Nothing found right after adding: the page goes on as if nothing was asked.
      if (msg.reason === "added" && msg.scan.plans.length === 0) return [set({ scanning: undefined }), []];
      const next = set({ scanning: undefined, found: listOf(msg.project, msg.scan) });
      if (msg.reason === "asked") return [next, []];
      return [{ ...next, plan: { ...next.plan, dialog: { kind: "foundPlans" } } }, []];
    }
    case "scanFailed":
      return [set({ scanning: undefined, scanError: msg.message }), []];
    case "togglePlan": {
      if (!f) return [s, []];
      const rest = f.selected.filter((p) => p !== msg.path);
      return [set({ found: { ...f, selected: msg.on ? [...rest, msg.path] : rest } }), []];
    }
    case "importSelected": {
      if (!f || importing(f)) return [s, []];
      const paths = f.plans.map((p) => p.path).filter((p) => f.selected.includes(p));
      if (!paths.length) return [s, []];
      const outcomes = { ...f.outcomes, ...Object.fromEntries(paths.map((p) => [p, { state: "running" } as PlanOutcome])) };
      return [set({ busy: true, found: { ...f, outcomes } }), [{ type: "importPlans", project: f.project, paths }]];
    }
    case "planImported": {
      const base = msg.result ? withImported(s, msg.result) : s;
      if (!f || f.project !== msg.project) return [base, []];
      const outcome = outcomeOf(msg);
      const done = outcome.state === "done";
      const plans = f.plans.map((p) => (p.path === msg.path && done ? { ...p, imported: true } : p));
      const selected = done ? f.selected.filter((p) => p !== msg.path) : f.selected;
      return [set({ found: { ...f, plans, selected, outcomes: { ...f.outcomes, [msg.path]: outcome } } }, base), []];
    }
    case "plansImported":
      return [set({ busy: false }), [{ type: "loadAgents" }]];
    case "skipPlans": {
      const next = set({ found: undefined });
      const closing = s.plan.dialog?.kind === "foundPlans";
      return [closing ? { ...next, plan: { ...next.plan, dialog: undefined } } : next, []];
    }
  }
}
