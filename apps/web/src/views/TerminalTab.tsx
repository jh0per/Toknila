import { useEffect, useRef, useState } from "react";
import type { Ticket } from "../api/types";
import { type TermStatus, openTerminal } from "../terminal/connect";
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
      const conn = openTerminal(card.key, {
        output: (bytes, reset) => {
          if (reset) term.reset();
          if (bytes) term.write(bytes);
        },
        status: (s) => {
          setStatus(s);
          if (s.kind === "live") setRefused("");
        },
        refused: setRefused,
      });
      const typing = new TextEncoder();
      const onData = term.onData((d) => conn.send(typing.encode(d)));
      dispose = () => {
        onData.dispose();
        resize.disconnect();
        conn.close();
        term.dispose();
      };
    })();
    return () => {
      cancelled = true;
      dispose();
    };
  }, [card.key]);

  return (
    <div className="terminal" data-testid="terminal">
      <div className="term-status" data-testid="term-status">
        <span className="muted">{STATUS_TEXT(status)}</span>
        {refused && <span className="error"> · {refused}</span>}
      </div>
      {status.kind === "none" && <p className="muted" data-testid="placeholder">No runner yet. Start the card to see its terminal.</p>}
      <div className="term-host" ref={host} data-testid="term-host" />
    </div>
  );
}
