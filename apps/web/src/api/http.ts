// The page's only network code: authenticated requests and the stream addresses.

import type { ErrorBody } from "./types";

/// The token comes from the address `tok open` prints: /?token=...
export function token(): string {
  return new URLSearchParams(window.location.search).get("token") ?? "";
}

const auth = () => ({ Authorization: `Bearer ${token()}` });

/// The daemon's error message for a refused request.
async function refusal(r: Response): Promise<Error> {
  const body = (await r.json().catch(() => ({}))) as Partial<ErrorBody>;
  return new Error(body.error ?? `${r.status} ${r.statusText}`);
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
export function runnerStreamUrl(key: string, cursor: { run: string; from: number } | null): string {
  const q = cursor ? `run=${encodeURIComponent(cursor.run)}&from=${cursor.from}&` : "";
  return `${wsBase()}/tickets/${encodeURIComponent(key)}/runner/stream?${q}token=${encodeURIComponent(token())}`;
}
