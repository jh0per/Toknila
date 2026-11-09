// How big a card of the diagram is. Pure: text is measured by a function the caller gives
// (`measureText.ts` in the window, a fake in tests). A card is as wide as its title and its
// status line need (196 to 320 px), its title wraps to two lines and is cut with an ellipsis only
// after the second, and its height follows the content. A person can resize a card by hand
// (196 to 480 px wide, up to 320 px tall); the size is then the card's own and still never
// smaller than what its content needs.

import type { Size } from "../api/types";

export const MIN_W = 196;
export const AUTO_MAX_W = 320;
export const RESIZE_MAX_W = 480;
export const RESIZE_MAX_H = 320;
const GRID = 8;

/// The three faces a card prints in; the measurer knows their fonts.
export type Face = "title" | "pill" | "key";
export type Measure = (text: string, face: Face) => number;

/// What sits under the status line: nothing, the small figures (subtasks, usage, pull request)
/// or a proposed card's Approve and Reject.
export type Foot = "none" | "extras" | "actions";

export interface CardContent {
  title: string;
  /// The status word on the pill (the fine status, never the group).
  pill: string;
  key: string;
  foot: Foot;
}

export interface CardLayout {
  w: number;
  h: number;
  /// The title, as drawn: one string per line, the last one cut with an ellipsis if it did not fit.
  lines: string[];
}

/// Space a card spends around its text, in pixels: 10 px padding and the border on each side,
/// 8 px padding, a 4 px top and a 1 px bottom border, and 3 px between rows.
const SIDE = 22;
const CHROME_Y = 21;
const GAP = 3;
const LINE_H = 16;
const PILL_H = 16;
/// The pill's padding and dot, the gap before the key, in pixels.
const PILL_EXTRA = 8 * 2 + 7 + 5;
const KEY_GAP = 6;
const FOOT_H: Record<Foot, number> = { none: 0, extras: 20, actions: 26 };
const ELLIPSIS = "…";

const up = (v: number): number => Math.ceil(v / GRID) * GRID;
const clamp = (v: number, lo: number, hi: number): number => Math.max(lo, Math.min(hi, v));

/// How many title lines a card of width `w` may show: two, and more when a person made it wide.
export const maxLines = (w: number): number => (w <= AUTO_MAX_W ? 2 : w <= 400 ? 3 : 4);

/// Breaks `text` into at most `max` lines no wider than `avail`, at spaces (a word wider than a
/// line is broken inside); the last line ends in an ellipsis when text was left over. Lines keep
/// the space they broke at, so they read back as the title.
export function wrapTitle(text: string, avail: number, max: number, m: Measure): string[] {
  const words = text.trim().split(/\s+/).filter(Boolean);
  const width = (s: string) => m(s, "title");
  const lines: string[] = [];
  let line = "";
  const queue = [...words];
  while (queue.length > 0) {
    const word = queue.shift() as string;
    const next = line ? `${line} ${word}` : word;
    if (width(next) <= avail) {
      line = next;
      continue;
    }
    if (line) {
      lines.push(`${line} `);
      line = "";
      queue.unshift(word);
      continue;
    }
    // One word wider than a line: take as much of it as fits.
    let n = word.length - 1;
    while (n > 1 && width(word.slice(0, n)) > avail) n--;
    lines.push(word.slice(0, Math.max(1, n)));
    queue.unshift(word.slice(Math.max(1, n)));
  }
  if (line) lines.push(line);
  if (lines.length <= max) return lines.length ? lines : [""];
  let last = lines.slice(max - 1).join("").trimEnd();
  while (last.length > 1 && width(last + ELLIPSIS) > avail) last = last.slice(0, -1).trimEnd();
  return [...lines.slice(0, max - 1), last + ELLIPSIS];
}

const lineWidth = (title: string, m: Measure): number => m(title.trim(), "title");

/// The width a card needs: its title on one line and its status line (pill and key), clamped.
export function autoWidth(c: CardContent, m: Measure): number {
  const status = m(c.pill, "pill") + PILL_EXTRA + KEY_GAP + m(c.key, "key");
  return clamp(up(Math.max(lineWidth(c.title, m), status) + SIDE), MIN_W, AUTO_MAX_W);
}

/// The height a card needs for `lines` title lines.
export const contentHeight = (lines: number, foot: Foot): number =>
  up(CHROME_Y + lines * LINE_H + GAP + PILL_H + (foot === "none" ? 0 : GAP + FOOT_H[foot]));

/// The card as its content makes it.
export function autoCard(c: CardContent, m: Measure): CardLayout {
  const w = autoWidth(c, m);
  const lines = wrapTitle(c.title, w - SIDE, maxLines(w), m);
  return { w, h: contentHeight(lines.length, c.foot), lines };
}

/// What a resize may not go below at width `w`: the height the wrapped title and the rows need.
export function minHeight(c: CardContent, w: number, m: Measure): number {
  const lines = wrapTitle(c.title, w - SIDE, maxLines(w), m);
  return Math.min(RESIZE_MAX_H, contentHeight(lines.length, c.foot));
}

/// A size a person asked for, on the grid and inside the limits: 196 to 480 wide, from `minH` to
/// 320 tall.
export function clampSize(s: Size, minH: number): Size {
  const snap = (v: number) => Math.round(v / GRID) * GRID;
  return {
    w: clamp(snap(s.w), MIN_W, RESIZE_MAX_W),
    h: clamp(snap(s.h), Math.min(minH, RESIZE_MAX_H), RESIZE_MAX_H),
  };
}

/// The card at the size a person gave it. Its title wraps to the width it has.
export function sizedCard(c: CardContent, s: Size, m: Measure): CardLayout {
  const w = clamp(Math.round(s.w), MIN_W, RESIZE_MAX_W);
  const lines = wrapTitle(c.title, w - SIDE, maxLines(w), m);
  const h = clamp(Math.round(s.h), Math.min(contentHeight(lines.length, c.foot), RESIZE_MAX_H), RESIZE_MAX_H);
  return { w, h, lines };
}
