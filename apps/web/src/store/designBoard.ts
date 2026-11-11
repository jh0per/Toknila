// The Designs view of a chain, as pure rules: which designs the board shows, where each tile sits,
// where a card's label sits on a design, which card a dropped handle lands on and what moving a
// reference does to the two cards' lists. Nothing here reads the page or calls the daemon.

import type { Design, Ticket } from "../api/types";
import { type Ref, type Region } from "./designs";

/// Width of a tile's picture at 100%, in pixels.
export const TILE_W = 520;
/// Space between tiles, and around the board.
export const GAP = 48;
export const PAD = 32;
/// Height of a tile's caption (the design's name).
export const CAPTION = 32;
/// Most tiles in a row.
export const COLUMNS = 3;
/// What the whole picture is when a card's reference names no region.
export const WHOLE: Region = { x: 0, y: 0, w: 100, h: 100 };

/// One card's use of a design: whose it is, how it stands and the part of the picture it is about.
export interface Use {
  /// The card's key (`DEMO-3`) and id.
  card: string;
  ticket: string;
  status: Ticket["status"];
  /// The reference as the card keeps it; `region` is `null` for the whole picture.
  region: Region | null;
}

/// A design on the board with every card that references it. `design` is `undefined` when the
/// project's library no longer has it.
export interface Item {
  id: string;
  name: string;
  design: Design | undefined;
  uses: Use[];
}

/// The designs of a chain's cards, each once, in the order the cards first name them. Cancelled
/// cards are left out: their mockups are not part of the plan any more.
export function boardItems(tickets: Ticket[], library: Design[]): Item[] {
  const items = new Map<string, Item>();
  for (const t of tickets) {
    if (t.status === "cancelled") continue;
    for (const r of t.designs ?? []) {
      const design = library.find((d) => d.id === r.design);
      const item = items.get(r.design) ?? { id: r.design, name: design?.name ?? r.design, design, uses: [] };
      item.uses.push({ card: t.key, ticket: t.id, status: t.status, region: r.region ?? null });
      items.set(r.design, item);
    }
  }
  return [...items.values()];
}

/// Whether the board draws the design as a picture: only a kept file that can be shown as one.
export const isPicture = (d: Design | undefined): boolean => Boolean(d?.thumb && d.has_file);

/// A tile's place on the board, in pixels at 100%: `h` is the picture's height, the caption comes
/// under it.
export interface Tile {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

/// The board: its tiles and its size.
export interface Board {
  tiles: Tile[];
  width: number;
  height: number;
}

/// The aspect (width over height) a picture is laid out with until it is known, and its bounds, so
/// a thin or a huge file cannot break the board.
export const DEFAULT_ASPECT = 4 / 3;
const clampAspect = (a: number | undefined): number =>
  a !== undefined && Number.isFinite(a) && a > 0 ? Math.min(4, Math.max(0.25, a)) : DEFAULT_ASPECT;

/// The tiles in rows of up to [`COLUMNS`], left to right, each row as tall as its tallest tile.
/// `aspects` holds the pictures' aspect once they have loaded.
export function layoutBoard(ids: string[], aspects: Record<string, number>): Board {
  const tiles: Tile[] = [];
  let y = PAD;
  let widest = 0;
  for (let row = 0; row < ids.length; row += COLUMNS) {
    const ofRow = ids.slice(row, row + COLUMNS);
    const heights = ofRow.map((id) => TILE_W / clampAspect(aspects[id]));
    ofRow.forEach((id, i) => tiles.push({ id, x: PAD + i * (TILE_W + GAP), y, w: TILE_W, h: heights[i] }));
    widest = Math.max(widest, ofRow.length);
    y += Math.max(...heights) + CAPTION + GAP;
  }
  if (ids.length === 0) return { tiles, width: 0, height: 0 };
  return { tiles, width: PAD * 2 + widest * TILE_W + (widest - 1) * GAP, height: y - GAP + PAD };
}

/// The region a use is drawn with: its own, else the whole picture.
export const regionOf = (u: Use): Region => u.region ?? WHOLE;

/// For each use of a tile, how many earlier uses start at the same corner: their labels would sit
/// on top of each other, so the view shifts each one along by its slot.
export function labelSlots(uses: Use[]): number[] {
  const seen = new Map<string, number>();
  return uses.map((u) => {
    const r = regionOf(u);
    const k = `${r.x},${r.y}`;
    const n = seen.get(k) ?? 0;
    seen.set(k, n + 1);
    return n;
  });
}

/// A box on the screen that a handle can be dropped on, for one card.
export interface DropBox {
  card: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

/// The card a drop at `at` lands on: of the boxes that hold the point the smallest one, so a
/// card's label wins over the picture region it sits in. `undefined` for a drop on nothing, and
/// for the card the handle came from.
export function dropTarget(boxes: DropBox[], at: { x: number; y: number }, from: string): string | undefined {
  const hit = boxes
    .filter((b) => b.card !== from && at.x >= b.x && at.x <= b.x + b.w && at.y >= b.y && at.y <= b.y + b.h)
    .sort((a, b) => a.w * a.h - b.w * b.h)[0];
  return hit?.card;
}

const sameRegion = (a: Region | null | undefined, b: Region | null | undefined): boolean =>
  (!a && !b) || (!!a && !!b && a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h);

const sameRef = (a: Ref, b: Ref): boolean => a.design === b.design && sameRegion(a.region, b.region);

/// Where `ref` stands in a card's list, the index the daemon's move route takes; `undefined` when
/// the card does not hold it. The daemon does the move (the other card gets it at the end unless it
/// has exactly that reference already).
export function refIndex(refs: Ref[], ref: Ref): number | undefined {
  const at = refs.findIndex((r) => sameRef(r, ref));
  return at < 0 ? undefined : at;
}
