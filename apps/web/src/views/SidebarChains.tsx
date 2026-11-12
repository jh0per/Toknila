import { useEffect, useState } from "react";
import type { ChainDetail } from "../api/types";
import type { Intent } from "../store/msg";
import { dayText, readFinishedOpen, splitChains, storeFinishedOpen, versionText } from "../store/phase";
import { PhaseDot } from "./PhaseChip";
import { NavLink } from "./Link";
import { Chev, type Level, indent } from "./TreeRow";

type Props = {
  chains: ChainDetail[];
  project: string;
  selected: string | undefined;
  dispatch: (i: Intent) => void;
  onMenu: (c: ChainDetail) => (e: React.MouseEvent) => void;
};

/// The chain rows of a project, as tree items one level under its Chains row: the unfinished ones
/// with a dot, the phase word and the count, then one collapsible Finished (n) group, closed by
/// default and remembered per project. The selected chain stays visible: the group opens for a
/// finished one.
export function SidebarChains({ chains, project, selected, dispatch, onMenu }: Props) {
  const { open, finished } = splitChains(chains);
  const [stored, setStored] = useState(() => readFinishedOpen(project));
  useEffect(() => setStored(readFinishedOpen(project)), [project]);
  const forced = finished.some((c) => c.chain.id === selected);
  const shown = stored || forced;
  const toggle = () => {
    const next = !shown;
    setStored(next);
    storeFinishedOpen(project, next);
  };
  const row = (c: ChainDetail, dim: boolean, level: Level) => {
    const s = c.summary;
    return (
      <li key={c.chain.id} role="treeitem" aria-selected={c.chain.id === selected} data-chain-item={c.chain.name} data-phase={s.phase} onContextMenu={onMenu(c)}>
        <div className="row" style={indent(level)}>
          <Chev />
          <NavLink
            route={{ name: "projectChain", project, chain: c.chain.name }}
            dispatch={dispatch} row title={c.chain.name}
            className={`item${c.chain.id === selected ? " sel" : ""}${dim ? " dim" : ""}`}
          >
            <PhaseDot phase={s.phase} />
            <span className="t key">{c.chain.name}</span>
            {s.version && <span className="ver" data-testid="chain-version-row">{versionText(s)}</span>}
            <span className="phase-word">{s.phase}</span>
            {dim && s.finished_at != null && <span className="n">{dayText(s.finished_at)}</span>}
            <span className="n">{s.done}/{s.total}</span>
          </NavLink>
        </div>
      </li>
    );
  };
  return (
    <>
      {open.map((c) => row(c, false, 2))}
      {finished.length > 0 && (
        <li
          role="treeitem" aria-expanded={shown} className="finished-group" data-testid="finished-group"
          data-open={shown || undefined}
        >
          <div className="row" style={indent(2)}>
            <Chev open={shown} />
            <button type="button" className="item group-toggle" data-row data-toggle data-testid="finished-toggle" onClick={toggle}>
              <span className="t">Finished ({finished.length})</span>
            </button>
          </div>
          {shown && <ul role="group">{finished.map((c) => row(c, true, 3))}</ul>}
        </li>
      )}
    </>
  );
}
