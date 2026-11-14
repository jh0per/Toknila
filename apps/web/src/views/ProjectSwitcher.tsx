import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import type { Route } from "../store/route";
import { type State, currentProject } from "../store/state";
import { switchRoute, switcherProjects } from "../store/switcher";
import { agentsOf, waitingIn } from "../store/tree";
import { projectBadge } from "./SidebarProject";

/// The crumb's project segment: the project's badge and name with a chevron. It opens the menu of
/// projects (`SwitcherMenu`, which the window renders once).
export function SwitcherButton({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  return (
    <button
      type="button" className="switcher-btn" data-testid="project-switcher" aria-haspopup="menu" aria-expanded={state.switcherOpen}
      title="Switch project (Cmd/Ctrl+K)" onClick={() => dispatch({ type: "toggleSwitcher" })}
    >
      {projectBadge(project)}
      <b>{project.name}</b>
      <span className="chev" aria-hidden="true">▾</span>
    </button>
  );
}

type Row = { id: string; label: string; run: () => void; project?: Project; hidden?: boolean };

/// The menu of projects, under the crumb's button: a filter field, "All projects", one row per
/// listed project, then the hidden ones under a "Hidden" divider (badge, name, a dot while an agent
/// runs, what waits in its inbox; the one on the screen is marked) and "Add project…". Up and Down
/// move, Enter picks, Escape closes; choosing a project opens the same screen there when it exists,
/// its roadmap otherwise.
export function SwitcherMenu({ state, dispatch, initialFilter = "" }: {
  state: State;
  dispatch: (i: Intent) => void;
  /// The text the filter starts with (the menu itself starts blank).
  initialFilter?: string;
}) {
  const [text, setText] = useState(initialFilter);
  const [at, setAt] = useState(0);
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number }>({ left: 16, top: 48 });
  const current = currentProject(state)?.key;
  const close = () => dispatch({ type: "closeSwitcher" });
  const go = (route: Route) => {
    close();
    dispatch({ type: "navigate", route });
  };
  const { listed, hidden } = switcherProjects(state, text);
  const projectRow = (isHidden: boolean) => (p: Project): Row => ({
    id: p.key, label: p.name, project: p, hidden: isHidden,
    run: () => go(switchRoute(state.route, p.key)),
  });
  const rows: Row[] = [
    { id: "all", label: "All projects", run: () => go({ name: "settings" }) },
    ...listed.map(projectRow(false)),
    ...hidden.map(projectRow(true)),
    { id: "add", label: "Add project…", run: () => { close(); dispatch({ type: "openDialog", dialog: { kind: "addProject" } }); } },
  ];
  const firstHidden = rows.findIndex((r) => r.hidden);
  const active = Math.min(at, rows.length - 1);

  // Under the button the menu opened from; a click outside or Escape closes it.
  useEffect(() => {
    const box = document.querySelector("[data-testid=project-switcher]")?.getBoundingClientRect();
    if (box) setPos({ left: Math.max(4, box.left), top: box.bottom + 4 });
  }, []);
  useEffect(() => {
    const down = (e: MouseEvent) => {
      const t = e.target as Element | null;
      if (!ref.current?.contains(t) && !t?.closest("[data-testid=project-switcher], [data-testid=sidebar-more]")) close();
    };
    window.addEventListener("mousedown", down);
    return () => window.removeEventListener("mousedown", down);
  });

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setAt((active + (e.key === "ArrowDown" ? 1 : rows.length - 1)) % rows.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      rows[active]?.run();
    }
  };

  return (
    <div
      ref={ref} className="ctx-menu switcher" role="menu" aria-label="Projects" data-testid="switcher-menu"
      style={pos} onKeyDown={onKeyDown}
    >
      <input
        className="switcher-filter" type="text" placeholder="Filter projects…" aria-label="Filter projects" data-testid="switcher-filter"
        autoFocus value={text} onChange={(e) => { setText(e.target.value); setAt(1); }}
      />
      {rows.map((r, i) => {
        const p = r.project;
        const running = p ? agentsOf(state, p).runs.length > 0 : false;
        const waiting = p ? waitingIn(state, p) : 0;
        return (
          <div key={r.id} role="none" className="ctx-item">
            {(r.id === "add" || (i === 1 && p && !r.hidden)) && <div className="ctx-sep" role="separator" />}
            {i === firstHidden && <div className="ctx-divider" role="separator" aria-label="Hidden" data-testid="switcher-hidden">Hidden</div>}
            <button
              type="button" role="menuitem" className={i === active ? "on" : ""} data-testid={`switcher-${p ? `item-${p.key}` : r.id}`}
              data-current={p && p.key === current ? "true" : undefined} aria-current={p && p.key === current ? "true" : undefined}
              onMouseEnter={() => setAt(i)} onClick={r.run}
            >
              {p ? projectBadge(p) : <span className="switcher-plus" aria-hidden="true">{r.id === "add" ? "+" : "◫"}</span>}
              <span className="ctx-label">{r.label}</span>
              {running && <span className="dot pulse" role="img" aria-label="An agent is running" data-testid="switcher-running" />}
              {waiting > 0 && <span className="badge" data-testid="switcher-waiting">{waiting}</span>}
              {p && p.key === current && <span className="ctx-check" aria-hidden="true">✓</span>}
            </button>
          </div>
        );
      })}
    </div>
  );
}
