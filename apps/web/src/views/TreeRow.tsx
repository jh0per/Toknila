import type { CSSProperties } from "react";

/// How deep a row of the sidebar's tree sits: the project is 0, its rows 1, a chain 2.
export type Level = 0 | 1 | 2 | 3;

/// The style that indents a row by its level.
export const indent = (level: Level): CSSProperties => ({ "--lv": level }) as CSSProperties;

/// The mark before a row's name: ▾ open, ▸ closed, nothing (but the room) for a leaf (`open`
/// absent). It is a picture; the row's own control opens and closes.
export function Chev({ open }: { open?: boolean }) {
  return <span className="chev" aria-hidden="true">{open === undefined ? "" : open ? "▾" : "▸"}</span>;
}
