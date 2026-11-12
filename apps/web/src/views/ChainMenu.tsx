import { type MouseEvent, type ReactNode, useCallback, useState } from "react";
import type { ChainDetail } from "../api/types";
import type { Intent } from "../store/msg";
import { AUTOSTART_HINT } from "../store/autostart";
import { ContextMenu } from "./ContextMenu";
import { menuGroups } from "./menuGroups";

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
    e.stopPropagation();
    setAt({ x: e.clientX, y: e.clientY, chain });
  };
  const menu = at && (
    <ContextMenu
      x={at.x} y={at.y} onClose={close}
      title={at.chain.chain.name}
      entries={menuGroups([{
        label: at.chain.chain.autostart ? "Stop starting stories automatically" : "Start stories automatically",
        testId: "menu-autostart",
        hint: AUTOSTART_HINT,
        run: () => dispatch({ type: "setAutostart", chain: at.chain.chain.id, autostart: !at.chain.chain.autostart }),
      }], [{
        label: "Delete epic…",
        testId: "menu-delete-chain",
        danger: true,
        run: () => dispatch({ type: "openDialog", dialog: { kind: "deleteChain", chain: at.chain.chain.id } }),
      }])}
    />
  );
  return { open, menu };
}
