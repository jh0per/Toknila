// Types mirror the daemon's JSON (crates/daemon/src/api/dto.rs, crates/core/src/model).

export type TicketStatus =
  | "backlog" | "ready" | "running" | "waiting" | "blocked"
  | "review" | "merged" | "released" | "cancelled";

export type StatusReason =
  | { status: "waiting"; kind: "question" | "permission"; text: string }
  | { status: "waiting"; kind: "silent" }
  | { status: "blocked"; reason: BlockedReason; resume_to: TicketStatus }
  | { status: "cancelled"; text: string };

export type BlockedReason =
  | { kind: "manual"; text: string }
  | { kind: "crashed"; detail: string }
  | { kind: "dependency_reopened"; dependency: string }
  | { kind: "merge_conflict"; detail: string }
  | { kind: "runner_lost" };

export interface Ticket {
  id: string;
  key: string;
  title: string;
  status: TicketStatus;
  status_reason: StatusReason | null;
  status_since: number;
  paused: boolean;
  profile: string | null;
}

export interface Release {
  id: string;
  version: string;
  channel: string;
  start_mode: "auto" | "manual";
  state: "planned" | "cut" | "released";
}

export type NodeRef = { kind: "ticket" | "release"; id: string };

export interface ChainSummary {
  status: string;
  total: number;
  done: number;
  released: number;
  stopped_at: { ticket: string; status: TicketStatus; reason: StatusReason | null; waiting_ms: number } | null;
  next_release: { version: string; state: string } | null;
}

export interface Chain { id: string; name: string; goal: string; autostart: boolean }

export interface ChainDetail {
  chain: Chain;
  tickets: Ticket[];
  releases: Release[];
  edges: { from: NodeRef; to: NodeRef; kind: "normal" | "after_release" }[];
  order: NodeRef[];
  summary: ChainSummary;
}

export interface TicketItem { ticket: Ticket; chain: string; in_status_ms: number; needs_human: boolean }

export interface EventRecord { seq: number; entity: string; kind: string }

/// The token comes from the address `tok open` prints: /?token=...
export function token(): string {
  return new URLSearchParams(window.location.search).get("token") ?? "";
}

export async function get<T>(path: string): Promise<T> {
  const r = await fetch(`/v1${path}`, { headers: { Authorization: `Bearer ${token()}` } });
  if (!r.ok) {
    const body = await r.json().catch(() => ({}));
    throw new Error(body.error ?? `${r.status} ${r.statusText}`);
  }
  return r.json() as Promise<T>;
}

export function streamUrl(since: number): string {
  const proto = window.location.protocol === "https:" ? "wss" : "ws";
  return `${proto}://${window.location.host}/v1/stream?since=${since}&token=${encodeURIComponent(token())}`;
}

export function reasonText(r: StatusReason | null): string {
  if (!r) return "";
  switch (r.status) {
    case "waiting":
      return r.kind === "silent" ? "agent went silent" : `${r.kind}: ${r.text}`;
    case "cancelled":
      return `cancelled: ${r.text}`;
    case "blocked": {
      const b = r.reason;
      switch (b.kind) {
        case "manual": return `blocked: ${b.text}`;
        case "crashed": return `crashed: ${b.detail}`;
        case "dependency_reopened": return `dependency reopened: ${b.dependency}`;
        case "merge_conflict": return `merge conflict: ${b.detail}`;
        case "runner_lost": return "runner lost";
      }
    }
  }
  return "";
}

export function duration(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 48) return `${h}h ${m % 60}m`;
  return `${Math.floor(h / 24)}d`;
}
