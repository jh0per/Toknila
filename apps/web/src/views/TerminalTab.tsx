import { useEffect, useRef, useState } from "react";
import type { Ticket } from "../api/types";
import { type TermStatus, openTerminal } from "../terminal/connect";
import { fitTerminal } from "../terminal/fit";
import { type SizeReporter, sizeReporter } from "../terminal/size";
import { exitText } from "../terminal/wire";

const STATUS_TEXT = (s: TermStatus): string => {
  switch (s.kind) {
    case "connecting": return "connecting…";
    case "none": return "no runner yet";
    case "live": return "live";
    case "ended": return `ended · ${exitText(s.exit)}`;
    case "offline": return "offline, reconnecting…";
  }
};

/// The card's runner terminal: xterm.js (loaded on first use) over the scrollback and the stream.
export function TerminalTab({ card }: { card: Ticket }) {
  const host = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<TermStatus>({ kind: "connecting" });
  const [refused, setRefused] = useState("");
  const [role, setRole] = useState<"worker" | "reviewer">("worker");

  useEffect(() => {
    let dispose = () => {};
    let cancelled = false;
    void (async () => {
      const [{ Terminal }] = await Promise.all([import("@xterm/xterm"), import("@xterm/xterm/css/xterm.css")]);
      const el = host.current;
      if (cancelled || !el) return;
      const css = getComputedStyle(document.documentElement);
      const term = new Terminal({
        fontFamily: css.getPropertyValue("--mono").trim() || "monospace",
        fontSize: 12,
        theme: { background: css.getPropertyValue("--term-bg").trim(), foreground: css.getPropertyValue("--term-fg").trim() },
      });
      term.open(el);
      // The grid follows the host's box: on every resize of the panel or the window, and once the
      // font is loaded (the cell size is known only then).
      const fit = () => fitTerminal(el, term);
      fit();
      const resize = new ResizeObserver(fit);
      resize.observe(el);
      void document.fonts?.ready.then(() => !cancelled && fit());
      // The agent draws for the grid the person sees: tell the daemon the panel's size when it
      // changes, and again whenever the stream goes live (a new run starts at the default size).
      let sizes: SizeReporter | null = null;
      setStatus({ kind: "connecting" });
      const conn = openTerminal(card.key, {
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
        },
        refused: setRefused,
      }, role);
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
  }, [card.key, role]);

  return (
    <div className="terminal" data-testid="terminal">
      <div className="term-status" data-testid="term-status">
        {(["worker", "reviewer"] as const).map((r) => (
          <button key={r} className={role === r ? "primary" : ""} data-testid={`term-role-${r}`} onClick={() => setRole(r)}>
            {r === "worker" ? "Worker" : "Reviewer"}
          </button>
        ))}{" "}
        <span className="muted">{STATUS_TEXT(status)}</span>
        {refused && <span className="error"> · {refused}</span>}
      </div>
      {status.kind === "none" && <p className="muted" data-testid="placeholder">No runner yet. Start the card to see its terminal.</p>}
      <div className="term-host" ref={host} data-testid="term-host" />
    </div>
  );
}
