import { type FocusEvent, type KeyboardEvent, useEffect, useRef } from "react";
import { stepIndex, treeMove } from "../store/tree";

const ROWS = "[data-row]";
const ITEM = 'li[role="treeitem"]';

/// The keyboard of a tree. Every row is an element marked `data-row` inside a `li[role=treeitem]`
/// (`aria-expanded` when it opens); an element marked `data-toggle` in the item's own row opens
/// and closes it. Exactly one row is a tab stop (the last one focused, else the first); Up, Down,
/// Home and End move, Right opens or enters, Left closes or goes to the parent. Spread the result
/// on the element that holds the trees.
export function useTree<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  const active = useRef<Element | null>(null);
  const rows = () => Array.from(ref.current?.querySelectorAll<HTMLElement>(ROWS) ?? []);
  const sync = (stop?: Element | null) => {
    const all = rows();
    const keep = stop ?? (active.current && all.includes(active.current as HTMLElement) ? active.current : all[0]);
    for (const r of all) r.tabIndex = r === keep ? 0 : -1;
  };
  useEffect(() => sync());
  const own = (item: Element, sel: string) => item.querySelector<HTMLElement>(`:scope > .row ${sel}`);
  return {
    ref,
    onFocus: (e: FocusEvent) => {
      if (!(e.target instanceof HTMLElement) || !e.target.matches(ROWS)) return;
      active.current = e.target;
      sync(e.target);
    },
    onKeyDown: (e: KeyboardEvent) => {
      const from = e.target instanceof HTMLElement && e.target.matches(ROWS) ? e.target : undefined;
      const item = from?.closest(ITEM);
      if (!from || !item || e.altKey || e.ctrlKey || e.metaKey) return;
      const expanded = item.hasAttribute("aria-expanded") ? item.getAttribute("aria-expanded") === "true" : undefined;
      const move = treeMove(e.key, expanded);
      if (!move) return;
      e.preventDefault();
      const all = rows();
      const to = stepIndex(move, all.indexOf(from), all.length);
      if (to !== undefined) return all[to]?.focus();
      if (move === "open" || move === "close") return own(item, "[data-toggle]")?.click();
      if (move === "child") return item.querySelector<HTMLElement>(`:scope > ul ${ROWS}`)?.focus();
      const parent = item.parentElement?.closest(ITEM);
      if (parent) own(parent, ROWS)?.focus();
    },
  };
}
