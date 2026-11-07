import { useEffect, useRef, useState } from "react";
import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { plannerOf } from "../store/planner";
import type { State } from "../store/state";
import { type TermStatus, openPlannerTerminal } from "../terminal/connect";
import { type SizeReporter, sizeReporter } from "../terminal/size";
import { exitText } from "../terminal/wire";

const STATUS_TEXT = (s: TermStatus): string => {
  switch (s.kind) {
    case "connecting": return "connecting…";
    case "none": return "not started yet";
    case "live": return "live";
    case "ended": return `ended · ${exitText(s.exit)}`;
    case "offline": return "offline, reconnecting…";
  }
};

/// The planner's live terminal: xterm.js (loaded on first use) over the scrollback and the
/// stream of the project's planner. The owner types to the planner here.
function PlannerTerminal({ project, onEnded }: { project: string; onEnded: () => void }) {
  const host = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<TermStatus>({ kind: "connecting" });
  const [refused, setRefused] = useState("");
  const ended = useRef(onEnded);
  ended.current = onEnded;

  useEffect(() => {
    let dispose = () => {};
    let cancelled = false;
    void (async () => {
      const [{ Terminal }, { FitAddon }] = await Promise.all([
        import("@xterm/xterm"),
        import("@xterm/addon-fit"),
        import("@xterm/xterm/css/xterm.css"),
      ]);
      const el = host.current;
      if (cancelled || !el) return;
      const css = getComputedStyle(document.documentElement);
      const term = new Terminal({
        fontFamily: css.getPropertyValue("--mono").trim() || "monospace",
        fontSize: 12,
        theme: { background: css.getPropertyValue("--term-bg").trim(), foreground: css.getPropertyValue("--term-fg").trim() },
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      term.open(el);
      fit.fit();
      const resize = new ResizeObserver(() => fit.fit());
      resize.observe(el);
      // The planner draws for the grid the person sees: tell the daemon the panel's size when it
      // changes, and again whenever the stream goes live (a new run starts at the default size).
      let sizes: SizeReporter | null = null;
      setStatus({ kind: "connecting" });
      const conn = openPlannerTerminal(project, {
        output: (bytes, reset) => {
          if (reset) term.reset();
          if (bytes) term.write(bytes);
        },
        status: (s) => {
          setStatus(s);
          if (s.kind === "live") {
            setRefused("");
            sizes?.resend();
          }
          if (s.kind === "ended") ended.current();
        },
        refused: setRefused,
      });
      sizes = sizeReporter((s) => conn.resize(s));
      sizes.report({ cols: term.cols, rows: term.rows });
      const onResize = term.onResize((s) => sizes?.report(s));
      const typing = new TextEncoder();
      const onData = term.onData((d) => conn.send(typing.encode(d)));
      dispose = () => {
        onData.dispose();
        onResize.dispose();
        sizes?.dispose();
        resize.disconnect();
        conn.close();
        term.dispose();
      };
    })();
    return () => {
      cancelled = true;
      dispose();
    };
  }, [project]);

  return (
    <div className="terminal" data-testid="terminal">
      <div className="term-status" data-testid="term-status">
        <span className="muted">{STATUS_TEXT(status)}</span>
        {refused && <span className="error"> · {refused}</span>}
      </div>
      {status.kind === "none" && (
        <p className="muted" data-testid="placeholder">The planner has not run yet. Start it to plan this project with it.</p>
      )}
      <div className="term-host" ref={host} data-testid="term-host" />
    </div>
  );
}

/// The bottom panel of the planner entry: Start and Stop, and the planner's terminal.
export function PlannerPanel({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const planner = plannerOf(state, project.key);
  const running = planner?.running ?? false;
  return (
    <section className="panel" data-testid="panel">
      <div className="tabs">
        <span className="who planner-who">
          <span className="key">Planner</span>
          <span className="muted" data-testid="planner-state">
            {planner === undefined ? "…" : running ? `running · ${planner.profile}` : "not running"}
          </span>
        </span>
        <span className="grow" />
        {running ? (
          <button type="button" className="btn" data-testid="planner-stop" onClick={() => dispatch({ type: "stopPlanner", project: project.key })}>
            Stop
          </button>
        ) : (
          <button
            type="button" className="btn primary" data-testid="planner-start" disabled={planner === undefined}
            onClick={() => dispatch({ type: "startPlanner", project: project.key })}
          >
            Start
          </button>
        )}
      </div>
      {state.planner.error && <p className="error side-note" role="alert" data-testid="planner-error">{state.planner.error}</p>}
      <div className="tabpanel" data-tab="planner">
        <PlannerTerminal
          key={project.key}
          project={project.key}
          onEnded={() => dispatch({ type: "refreshPlanner", project: project.key })}
        />
      </div>
    </section>
  );
}
