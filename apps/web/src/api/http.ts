// The page's only network code: authenticated GETs and the stream address.

import type { ErrorBody } from "./types";

/// The token comes from the address `tok open` prints: /?token=...
export function token(): string {
  return new URLSearchParams(window.location.search).get("token") ?? "";
}

export async function get<T>(path: string): Promise<T> {
  const r = await fetch(`/v1${path}`, { headers: { Authorization: `Bearer ${token()}` } });
  if (!r.ok) {
    const body = (await r.json().catch(() => ({}))) as Partial<ErrorBody>;
    throw new Error(body.error ?? `${r.status} ${r.statusText}`);
  }
  return r.json() as Promise<T>;
}

export function streamUrl(since: number): string {
  const proto = window.location.protocol === "https:" ? "wss" : "ws";
  return `${proto}://${window.location.host}/v1/stream?since=${since}&token=${encodeURIComponent(token())}`;
}
