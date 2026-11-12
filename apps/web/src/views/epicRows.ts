// The sidebar's epic rows as drop targets for a card dragged from the diagram. A row says which
// epic it is (`data-chain-id`) and which project it is in (`data-chain-project`); the card may be
// dropped on the rows of its own project except its own epic.

/// The epic row at `el` (the element under the pointer, or one inside a row): its chain id and the
/// key of its project; nothing when `el` is not in a row.
export function epicRowOf(el: Pick<Element, "closest"> | null | undefined): { chain: string; project: string } | undefined {
  const row = el?.closest("[data-chain-id]");
  const chain = row?.getAttribute("data-chain-id");
  const project = row?.getAttribute("data-chain-project");
  return chain && project ? { chain, project } : undefined;
}

/// The epic a card of `project` that lives in `own` would go to when dropped at `el`: the row's
/// chain id, or nothing for another project's row, its own epic's row, or no row.
export function dropTarget(el: Pick<Element, "closest"> | null | undefined, project: string, own: string): string | undefined {
  const row = epicRowOf(el);
  return row && row.project === project && row.chain !== own ? row.chain : undefined;
}
