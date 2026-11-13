// Pure helpers of the context menus: grouping with separators, the keys a menu answers, and
// type-ahead. No React here so the rules are tested alone.

export interface MenuEntry {
  label: string;
  testId: string;
  run: () => void;
  danger?: boolean;
  /// Shown as the entry's tooltip.
  hint?: string;
  /// The shortcut shown at the right of the entry. A single letter or `Delete` also works while
  /// the menu is open; `Enter` is the page's own key (it opens the card in focus), shown as a hint.
  keys?: string;
  /// A separator is drawn above this entry.
  sep?: boolean;
  /// For a choice among several: whether this one is the current choice (a check mark shows).
  checked?: boolean;
  /// Greyed and not run when picked (the epic a card is in, in the "Move to epic" chooser).
  disabled?: boolean;
}

/// The entries of `groups` in one list, with a separator between the groups; an empty group
/// leaves no separator behind.
export function menuGroups(...groups: MenuEntry[][]): MenuEntry[] {
  const out: MenuEntry[] = [];
  for (const g of groups) {
    if (g.length === 0) continue;
    out.push(...g.map((e, i) => (i === 0 && out.length > 0 ? { ...e, sep: true } : e)));
  }
  return out;
}

/// The entry a pressed key runs, by the shortcut shown at its right (`Enter` is not one: inside a
/// menu it activates the entry in focus).
export function shortcutEntry(entries: MenuEntry[], key: string): MenuEntry | undefined {
  if (key === "Enter") return undefined;
  return entries.find((e) => e.keys !== undefined && e.keys.toLowerCase() === key.toLowerCase());
}

/// Where the arrow keys, Home, End and Tab move the focus in a menu of `n` entries, from `at`
/// (-1 when none has it); undefined for any other key.
export function moveFocus(key: string, at: number, n: number, shift = false): number | undefined {
  if (n === 0) return undefined;
  switch (key) {
    case "ArrowDown":
      return (at + 1) % n;
    case "ArrowUp":
      return at <= 0 ? n - 1 : at - 1;
    case "Tab":
      return shift ? (at <= 0 ? n - 1 : at - 1) : (at + 1) % n;
    case "Home":
      return 0;
    case "End":
      return n - 1;
    default:
      return undefined;
  }
}

/// Type-ahead: the next entry (after `from`, wrapping) whose label starts with what was typed. A
/// second letter in a row keeps the entry in focus when it still matches.
export function typeAhead(labels: string[], typed: string, from: number): number | undefined {
  const t = typed.toLowerCase();
  const start = typed.length > 1 ? from - 1 : from;
  for (let step = 1; step <= labels.length; step++) {
    const i = (((start + step) % labels.length) + labels.length) % labels.length;
    if (labels[i].toLowerCase().startsWith(t)) return i;
  }
  return undefined;
}
