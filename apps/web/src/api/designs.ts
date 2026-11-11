// The designs routes whose bodies are not JSON, or whose answers are empty.

import { Refusal, token } from "./http";
import type { Design } from "./types";

async function refused(r: Response): Promise<Refusal> {
  const body = (await r.json().catch(() => ({}))) as { error?: string; code?: string; field?: string };
  return new Refusal(body.error ?? `${r.status} ${r.statusText}`, body.code ?? "", body.field);
}

const headers = (type?: string): HeadersInit => ({
  Authorization: `Bearer ${token()}`,
  ...(type ? { "Content-Type": type } : {}),
});

/// Uploads one file as a design: the raw bytes as the body, the name in the address.
export async function uploadDesign(project: string, file: File): Promise<Design> {
  const r = await fetch(`/v1/projects/${encodeURIComponent(project)}/designs?name=${encodeURIComponent(file.name)}`, {
    method: "POST",
    headers: headers("application/octet-stream"),
    body: file,
  });
  if (!r.ok) throw await refused(r);
  return (await r.json()) as Design;
}

/// A call whose answer has no body (204): removing a design, saving or forgetting the token.
export async function sendEmpty(method: "PUT" | "DELETE", path: string, body?: unknown): Promise<void> {
  const r = await fetch(`/v1${path}`, {
    method,
    headers: headers(body === undefined ? undefined : "application/json"),
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!r.ok) throw await refused(r);
}
