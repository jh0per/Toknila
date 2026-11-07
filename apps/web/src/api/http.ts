// The page's only network code: authenticated requests and the stream addresses.

import type { ErrorBody } from "./types";

/// The token comes from the address `tok open` prints: /?token=...
export function token(): string {
  return new URLSearchParams(window.location.search).get("token") ?? "";
}

const auth = () => ({ Authorization: `Bearer ${token()}` });

/// A refused request: the daemon's message, and the input it is about when it names one.
export class Refusal extends Error {
  constructor(message: string, readonly code: string, readonly field: string | undefined) {
    super(message);
  }
}

async function refusal(r: Response): Promise<Refusal> {
  const body = (await r.json().catch(() => ({}))) as Partial<ErrorBody>;
  return new Refusal(body.error ?? `${r.status} ${r.statusText}`, body.code ?? "", body.field ?? undefined);
}

/// A setting: PUT a JSON body, answer with the daemon's reply.
export async function put<T>(path: string, body: unknown): Promise<T> {
  const r = await fetch(`/v1${path}`, {
    method: "PUT",
    headers: { ...auth(), "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!r.ok) throw await refusal(r);
  return r.json() as Promise<T>;
}

export async function get<T>(path: string): Promise<T> {
  const r = await fetch(`/v1${path}`, { headers: auth() });
  if (!r.ok) throw await refusal(r);
  return r.json() as Promise<T>;
}

/// A command: POST a JSON body, throw the daemon's error message on refusal.
export async function post<T = void>(path: string, body: unknown): Promise<T> {
  const r = await fetch(`/v1${path}`, {
    method: "POST",
    headers: { ...auth(), "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!r.ok) throw await refusal(r);
  return (r.status === 204 ? undefined : await r.json().catch(() => undefined)) as T;
}

/// A change of settings: PATCH a JSON body, throw the daemon's refusal.
export async function patch<T>(path: string, body: unknown): Promise<T> {
  const r = await fetch(`/v1${path}`, {
    method: "PATCH",
    headers: { ...auth(), "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!r.ok) throw await refusal(r);
  return r.json() as Promise<T>;
}

/// A deletion: DELETE, answer with the daemon's reply or throw its refusal.
export async function del<T>(path: string): Promise<T> {
  const r = await fetch(`/v1${path}`, { method: "DELETE", headers: auth() });
  if (!r.ok) throw await refusal(r);
  return r.json() as Promise<T>;
}

/// Raw bytes typed into a terminal, as they are.
export async function postBytes(path: string, bytes: Uint8Array): Promise<void> {
  const r = await fetch(`/v1${path}`, {
    method: "POST",
    headers: { ...auth(), "Content-Type": "application/octet-stream" },
    body: bytes as BodyInit,
  });
  if (!r.ok) throw await refusal(r);
}

/// Raw bytes and the answer's headers; `null` when there is nothing yet (404).
export async function getBytes(path: string): Promise<{ bytes: ArrayBuffer; headers: Headers } | null> {
  const r = await fetch(`/v1${path}`, { headers: auth() });
  if (r.status === 404) return null;
  if (!r.ok) throw await refusal(r);
  return { bytes: await r.arrayBuffer(), headers: r.headers };
}

function wsBase(): string {
  return `${window.location.protocol === "https:" ? "wss" : "ws"}://${window.location.host}/v1`;
}

export function streamUrl(since: number): string {
  return `${wsBase()}/stream?since=${since}&token=${encodeURIComponent(token())}`;
}

/// A ticket's runner stream, continuing after `from` of `run` when the page already has that.
export function runnerStreamUrl(
  key: string,
  cursor: { run: string; from: number } | null,
  role: "worker" | "reviewer" = "worker",
): string {
  const q = (cursor ? `run=${encodeURIComponent(cursor.run)}&from=${cursor.from}&` : "") +
    (role === "reviewer" ? "role=reviewer&" : "");
  return `${wsBase()}/tickets/${encodeURIComponent(key)}/runner/stream?${q}token=${encodeURIComponent(token())}`;
}

/// A project's planner terminal stream, continuing after `from` of `run` when the page has that.
export function plannerStreamUrl(project: string, cursor: { run: string; from: number } | null): string {
  const q = cursor ? `run=${encodeURIComponent(cursor.run)}&from=${cursor.from}&` : "";
  return `${wsBase()}/projects/${encodeURIComponent(project)}/planner/stream?${q}token=${encodeURIComponent(token())}`;
}
