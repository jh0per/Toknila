import { type MouseEvent, type ReactNode, useCallback, useState } from "react";
import type { ChainDetail } from "../api/types";
import type { Intent } from "../store/msg";
import { ContextMenu } from "./ContextMenu";

/// The right-click menu of a chain in the sidebar. `open(chain)` returns the handler for a row;
/// render `menu` once next to the list.
export function useChainMenu(dispatch: (i: Intent) => void): {
  open: (chain: ChainDetail) => (e: MouseEvent) => void;
  menu: ReactNode;
} {
  const [at, setAt] = useState<{ x: number; y: number; chain: ChainDetail }>();
  const close = useCallback(() => setAt(undefined), []);
  const open = (chain: ChainDetail) => (e: MouseEvent) => {
    e.preventDefault();
    setAt({ x: e.clientX, y: e.clientY, chain });
  };
  const menu = at && (
    <ContextMenu
      x={at.x} y={at.y} onClose={close}
      entries={[{
        label: "Delete chain…",
        testId: "menu-delete-chain",
        danger: true,
        run: () => dispatch({ type: "openDialog", dialog: { kind: "deleteChain", chain: at.chain.chain.id } }),
      }]}
    />
  );
  return { open, menu };
}
