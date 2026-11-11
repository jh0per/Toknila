import type { Design } from "../api/types";

/// Most bytes the daemon keeps of one design file (the same bound as the daemon's).
export const MAX_DESIGN_BYTES = 25 * 1024 * 1024;

const TYPES = ["image/png", "image/jpeg", "image/webp", "application/pdf"];
const EXTENSIONS = [".png", ".jpg", ".jpeg", ".webp", ".pdf"];

/// Whether a dropped file looks like a design (the daemon decides again from the file's bytes).
export function looksLikeDesign(f: { name: string; type: string }): boolean {
  return TYPES.includes(f.type) || EXTENSIONS.some((e) => f.name.toLowerCase().endsWith(e));
}

/// The files of a drop that cannot be designs, as a sentence; empty when all can.
export function refusedFiles(files: { name: string; type: string; size: number }[]): string {
  const names = files.filter((f) => !looksLikeDesign(f)).map((f) => f.name);
  const big = files.filter((f) => f.size > MAX_DESIGN_BYTES).map((f) => f.name);
  const parts: string[] = [];
  if (names.length > 0) parts.push(`${names.join(", ")}: only png, jpg, webp and pdf files can be designs`);
  if (big.length > 0) parts.push(`${big.join(", ")}: larger than 25 MB`);
  return parts.join("; ");
}

/// What kind of design a card says it is.
export function kindLabel(d: Design): string {
  return d.kind === "figma" ? "Figma" : d.kind === "pdf" ? "PDF" : (d.mime ?? "image").replace("image/", "").toUpperCase();
}

/// A file size for a card: `12 KB`, `1.3 MB`.
export function sizeText(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/// The day a design was added, as `2026-11-11`.
export function dayText(ms: number): string {
  return new Date(ms).toISOString().slice(0, 10);
}

/// Where a design's kept file is served; the token rides in the address because an `<img>` cannot
/// send a header.
export function fileUrl(project: string, d: Design, token: string): string {
  return `/v1/projects/${encodeURIComponent(project)}/designs/${d.id}/file?token=${encodeURIComponent(token)}`;
}

/// What a card shows in place of a picture: how to get one for a link, or a mark for a pdf.
export function placeholder(d: Design): string {
  if (d.kind === "pdf") return "PDF";
  if (d.kind === "figma") return d.hint ?? "Export the frame from Figma as a png and drop it here.";
  return "";
}
