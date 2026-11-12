import { useEffect, useState } from "react";
import type { ChainDetail, Project } from "../api/types";
import type { Intent } from "../store/msg";
import { readFinishedOpen, storeFinishedOpen, versionText } from "../store/phase";
import { dropMove, moveStep, roadmapPhase, splitRoadmap } from "../store/roadmap";
import { type State, chainsOf } from "../store/state";
import { epicTitle } from "../store/title";
import { AssistantButton } from "./AssistantButton";
import { NavLink } from "./Link";
import { SwitcherButton } from "./ProjectSwitcher";
import { SpendLine } from "./SpendLine";
import { ThemeButton } from "./ThemeButton";

type Dispatch = (i: Intent) => void;

/// Drag is for a pointer; under 640px wide the menu is the only way to reorder.
const narrow = (): boolean => window.matchMedia("(max-width: 639px)").matches;

/// The project's page: its epics as a sequence from left to right, one node per chain in roadmap
/// order, the finished ones folded into one group at the start. A click opens the chain; a node
/// moves by its menu (Move earlier, Move later) or by drag; New epic sits at the end.
export function Roadmap({ project, state, dispatch }: { project: Project; state: State; dispatch: Dispatch }) {
  const { finished, rest } = splitRoadmap(chainsOf(state, project));
  const [stored, setStored] = useState(() => readFinishedOpen(project.key));
  useEffect(() => setStored(readFinishedOpen(project.key)), [project.key]);
  const [menu, setMenu] = useState<string | undefined>(undefined);
  const [drag, setDrag] = useState<{ id: string; over: string | undefined } | undefined>(undefined);
  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(undefined);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("click", close);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("keydown", esc);
    };
  }, [menu]);
  const toggle = () => {
    setStored(!stored);
    storeFinishedOpen(project.key, !stored);
  };
  const move = (to: ReturnType<typeof moveStep>, id: string) => {
    if (to) dispatch({ type: "moveChain", chain: id, project: project.key, to });
  };
  const node = (c: ChainDetail, shown: ChainDetail[]) => {
    const s = c.summary;
    const id = c.chain.id;
    const phase = roadmapPhase(s);
    const earlier = moveStep(shown, id, -1);
    const later = moveStep(shown, id, 1);
    const pct = s.total > 0 ? Math.round((s.done / s.total) * 100) : 0;
    return (
      <li
        key={id} className={`rm-node${drag?.id === id ? " dragging" : ""}${drag?.over === id && drag.id !== id ? " drop" : ""}`}
        data-testid="roadmap-node" data-chain={c.chain.name} data-phase={phase} data-active={phase === "active" || undefined}
        draggable
        onDragStart={(e) => {
          if (narrow()) return e.preventDefault();
          e.dataTransfer.effectAllowed = "move";
          e.dataTransfer.setData("text/plain", id);
          setDrag({ id, over: undefined });
        }}
        onDragOver={(e) => {
          if (!drag) return;
          e.preventDefault();
          if (drag.over !== id) setDrag({ ...drag, over: id });
        }}
        onDrop={(e) => {
          e.preventDefault();
          if (drag) move(dropMove(shown, drag.id, id), drag.id);
          setDrag(undefined);
        }}
        onDragEnd={() => setDrag(undefined)}
      >
        <NavLink route={{ name: "projectChain", project: project.key, chain: c.chain.name }} dispatch={dispatch} className="rm-body" testId="roadmap-open">
          <span className="rm-top">
            <span className="rm-name" data-testid="roadmap-name">{epicTitle(c)}</span>
            {s.version && <span className="ver" data-testid="roadmap-version">{versionText(s)}</span>}
          </span>
          <span className="rm-phase" data-testid="roadmap-phase">{phase}</span>
          <span className="rm-count" data-testid="roadmap-count">{s.done}/{s.total}</span>
          <span className="rm-bar" aria-hidden="true"><i style={{ width: `${pct}%` }} /></span>
        </NavLink>
        <button
          type="button" className="rm-kebab" aria-label={`Menu of ${epicTitle(c)}`} aria-haspopup="menu" aria-expanded={menu === id}
          data-testid="roadmap-menu-button"
          onClick={(e) => {
            e.stopPropagation();
            setMenu(menu === id ? undefined : id);
          }}
        >
          ⋮
        </button>
        {menu === id && (
          <div className="rm-menu" role="menu" data-testid="roadmap-menu">
            <button type="button" role="menuitem" disabled={!earlier} data-testid="roadmap-move-earlier" onClick={() => move(earlier, id)}>Move earlier</button>
            <button type="button" role="menuitem" disabled={!later} data-testid="roadmap-move-later" onClick={() => move(later, id)}>Move later</button>
          </div>
        )}
      </li>
    );
  };
  const join = (k: string, on?: boolean) => <li key={k} className={`rm-join${on ? " cur" : ""}`} aria-hidden="true" />;
  const items: React.ReactNode[] = [];
  if (finished.length > 0) {
    items.push(
      <li key="fold" className="rm-fold" data-testid="roadmap-finished" data-open={stored || undefined}>
        <button type="button" aria-expanded={stored} data-testid="roadmap-finished-toggle" onClick={toggle}>
          <b>{finished.length} finished</b>
          <small>{stored ? "click to fold" : "click to open"}</small>
        </button>
      </li>,
    );
    items.push(join("j-fold"));
    if (stored) {
      for (const c of finished) items.push(node(c, finished), join(`j-${c.chain.id}`));
    }
  }
  rest.forEach((c, i) => {
    items.push(node(c, rest));
    items.push(join(`j${i}`, c.summary.phase === "active" || c.summary.phase === "waiting"));
  });
  items.push(
    <li key="new" className="rm-new">
      <button
        type="button" data-testid="roadmap-new-epic"
        onClick={() => dispatch({ type: "openDialog", dialog: { kind: "newChain", project: project.key } })}
      >
        + New epic
      </button>
    </li>,
  );
  return (
    <>
      <header className="top">
        <div className="crumb">
          <SwitcherButton project={project} state={state} dispatch={dispatch} /><i>/</i><b data-testid="roadmap-title">Roadmap</b>
        </div>
        <SpendLine state={state} dispatch={dispatch} />
        <span className="muted">{finished.length + rest.length} epics</span>
        {state.error && <span className="error">{state.error}</span>}
        {state.roadmap.error && <span className="error" role="alert" data-testid="roadmap-error">{state.roadmap.error}</span>}
        <span className="grow" />
        <ThemeButton theme={state.theme} dispatch={dispatch} />
        <AssistantButton state={state} dispatch={dispatch} />
      </header>
      <div className="roadmap" data-testid="roadmap">
        <ol className="rm-road">{items}</ol>
      </div>
    </>
  );
}
