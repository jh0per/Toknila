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

/// Where a design's picture is served: the image itself, or the first page of a pdf. Only a
/// design with `thumb` has one.
export function thumbUrl(project: string, d: Design, token: string): string {
  return `/v1/projects/${encodeURIComponent(project)}/designs/${d.id}/thumb?token=${encodeURIComponent(token)}`;
}

/// Whether the design is an image the page can show large and mark regions on (a pdf's first page
/// is only its thumbnail; the pdf itself is read in the page).
export const isImage = (d: Design): boolean => d.thumb && d.kind !== "pdf";

/// What the window asks before it removes a design that is still in use: `Used by TOKNILA-12,
/// TOKNILA-14 and 2 pins. Remove anyway?`
export function usedText(u: { cards: string[]; pins: number }): string {
  const parts = [...u.cards];
  if (u.pins > 0) parts.push(`${u.pins} ${u.pins === 1 ? "pin" : "pins"}`);
  const used = parts.length > 1 ? `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}` : (parts[0] ?? "nothing");
  return `Used by ${used}. Remove anyway?`;
}

/// What a card shows in place of a picture: how to get one for a link, or a mark for a pdf.
export function placeholder(d: Design): string {
  if (d.kind === "pdf") return "PDF";
  if (d.kind === "figma") return d.hint ?? "Export the frame from Figma as a png and drop it here.";
  return "";
}

/// A card's reference to a design: the design's id and the part of its image that matters, in
/// whole percent of the image (none: the whole image).
export interface Ref {
  design: string;
  region?: Region | null;
}
export interface Region {
  x: number;
  y: number;
  w: number;
  h: number;
}

/// The percent box of a region, as a style: it scales with the picture it sits on.
export function regionStyle(r: Region): { left: string; top: string; width: string; height: string } {
  return { left: `${r.x}%`, top: `${r.y}%`, width: `${r.w}%`, height: `${r.h}%` };
}

/// The region a drag drew, from two corners given as fractions (0 to 1) of the picture: whole
/// percent, inside the picture, at least 1% wide and tall. `undefined` for a click that moved too
/// little to be a region.
export function dragRegion(a: { x: number; y: number }, b: { x: number; y: number }): Region | undefined {
  const pct = (v: number) => Math.max(0, Math.min(100, Math.round(v * 100)));
  const [x0, x1] = [pct(Math.min(a.x, b.x)), pct(Math.max(a.x, b.x))];
  const [y0, y1] = [pct(Math.min(a.y, b.y)), pct(Math.max(a.y, b.y))];
  if (x1 - x0 < 1 || y1 - y0 < 1) return undefined;
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

/// A region in words: `x 20%, y 10%, 40% wide, 30% tall`.
export function regionText(r: Region): string {
  return `x ${r.x}%, y ${r.y}%, ${r.w}% wide, ${r.h}% tall`;
}

/// The zoom steps of a mockup, in percent of the width of its frame.
export const ZOOMS = [50, 75, 100, 150, 200, 300, 400];

/// The next zoom step up or down from `zoom`, never outside [`ZOOMS`].
export function stepZoom(zoom: number, dir: 1 | -1): number {
  if (dir === 1) return ZOOMS.find((z) => z > zoom) ?? ZOOMS[ZOOMS.length - 1];
  return [...ZOOMS].reverse().find((z) => z < zoom) ?? ZOOMS[0];
}

/// The card's references with the one at `index` replaced (`ref` undefined: removed); `index`
/// beyond the end adds.
export function withRef(refs: Ref[], index: number, ref: Ref | undefined): Ref[] {
  const out = refs.map((r) => ({ ...r }));
  if (ref === undefined) out.splice(index, 1);
  else if (index >= out.length) out.push(ref);
  else out[index] = ref;
  return out;
}

/// The design of a reference, when the library still has it.
export function designOf(list: Design[], ref: Ref): Design | undefined {
  return list.find((d) => d.id === ref.design);
}
