import { type ReactNode, useEffect, useRef, useState } from "react";
import type { Terminal } from "@xterm/xterm";
import { writeClipboard } from "../terminal/clipboard";
import { type TermStatus, type TerminalConnection, type TerminalSink } from "../terminal/connect";
import { fitTerminal } from "../terminal/fit";
import { isMac, keyAction } from "../terminal/keys";
import { keepingSelection } from "../terminal/keepSelection";
import { decodeOsc52 } from "../terminal/osc52";
import { allText, visibleText } from "../terminal/screen";
import { TermCopy } from "./TermCopy";
import { currentRenderer } from "../terminal/renderer";
import { type SizeReporter, sizeReporter } from "../terminal/size";
import { exitText } from "../terminal/wire";

const statusText = (s: TermStatus, none: string): string => {
  switch (s.kind) {
    case "connecting": return "connecting…";
    case "none": return none;
    case "live": return "live";
    case "ended": return `ended · ${exitText(s.exit)}`;
    case "offline": return "offline, reconnecting…";
  }
};

/// An agent's live terminal: xterm.js (loaded on first use) over a connection to the daemon (the
/// scrollback, then the stream). The one place that sets the emulator up: it fits the grid to its
/// box on every resize of the panel or the window, tells the daemon the grid's size so the agent
/// draws for what the person sees, and sends the keystrokes. `connect` opens the connection
/// (a card's runner, a project's planner); a new `streamKey` reconnects. The component takes the
/// whole height its parent gives it.
export function LiveTerminal({ streamKey, connect, noneText, placeholder, offer, lead, onEnded }: {
  streamKey: string;
  connect: (sink: TerminalSink) => TerminalConnection;
  /// The status line while no run has written anything yet.
  noneText: string;
  /// Shown while there is no run.
  placeholder: string;
  /// What to do about it, shown with the placeholder. With it, the empty terminal is not drawn:
  /// the placeholder and this are centred until something has run.
  offer?: ReactNode;
  /// Controls before the status line (the card's worker/reviewer switch).
  lead?: ReactNode;
  /// The run ended (the stream said so).
  onEnded?: () => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<TermStatus>({ kind: "connecting" });
  const [refused, setRefused] = useState("");
  const termRef = useRef<Terminal | null>(null);
  // Text an agent asked to copy that the clipboard refused, until a click puts it there.
  const [pending, setPending] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const flash = useRef<number>();
  const put = async (text: string): Promise<boolean> => {
    const ok = await writeClipboard(text);
    if (ok) {
      setCopied(true);
      window.clearTimeout(flash.current);
      flash.current = window.setTimeout(() => setCopied(false), 1500);
    }
    return ok;
  };
  const copy = () => {
    const t = termRef.current;
    if (t) void put(t.hasSelection() ? t.getSelection() : visibleText(t.buffer.active, t.rows));
  };
  const copyAll = () => {
    const t = termRef.current;
    if (t) void put(allText(t.buffer.active));
  };
  const takeReady = () => {
    if (pending !== null) void put(pending).then((ok) => ok && setPending(null));
  };
  const deliver = useRef((_text: string) => {});
  deliver.current = (text) => void put(text).then((ok) => setPending(ok ? null : text));
  // The latest callbacks, so a re-render does not reconnect.
  const latest = useRef({ connect, onEnded });
  latest.current = { connect, onEnded };

  // `streamKey` is what picks the connection; the callbacks are read from `latest`.
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
        // A calm terminal: no blinking cursor, no animated scrolling, a bounded scrollback.
        cursorBlink: false,
        smoothScrollDuration: 0,
        scrollback: 5000,
        // The agent owns the mouse (tracking is on), so selecting is Option-drag on a Mac; Shift-drag
        // elsewhere is xterm's own.
        macOptionClickForcesSelection: true,
        theme: { background: css.getPropertyValue("--term-bg").trim(), foreground: css.getPropertyValue("--term-fg").trim() },
      });
      term.open(el);
      termRef.current = term;
      // The agent's own copy (OSC 52) goes to the clipboard; every OSC 52 is consumed, a query
      // included (the agent never reads the clipboard).
      const osc = term.parser.registerOscHandler(52, (data) => {
        const text = decodeOsc52(data);
        if (text !== null) deliver.current(text);
        return true;
      });
      // Cmd+C (Mac), Ctrl+Shift+C, and Ctrl+C with a selection copy; the agent does not get them.
      const mac = isMac(navigator.platform);
      term.attachCustomKeyEventHandler((e) => {
        if (e.type !== "keydown" || keyAction(e, mac, term.hasSelection()) !== "copy") return true;
        e.preventDefault();
        if (term.hasSelection()) void put(term.getSelection());
        return false;
      });
      // The canvas renderer draws a TUI's full-frame redraws without flicker; if WebGL is missing
      // or its context is lost, the DOM renderer (the default) carries on.
      if (currentRenderer() === "webgl") {
        try {
          const { WebglAddon } = await import("@xterm/addon-webgl");
          if (cancelled) return term.dispose();
          const gl = new WebglAddon();
          gl.onContextLoss(() => gl.dispose());
          term.loadAddon(gl);
        } catch {
          /* no WebGL: the DOM renderer stays */
        }
      }
      // The grid follows the host's box: on every resize of the panel or the window, and once the
      // font is loaded (the cell size is known only then).
      const fit = () => keepingSelection(term, () => fitTerminal(el, term));
      fit();
      const resize = new ResizeObserver(fit);
      resize.observe(el);
      void document.fonts?.ready.then(() => !cancelled && fit());
      // The agent draws for the grid the person sees: tell the daemon the panel's size when it
      // changes, and again whenever the stream goes live (a new run starts at the default size).
      let sizes: SizeReporter | null = null;
      setStatus({ kind: "connecting" });
      const conn = latest.current.connect({
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
          if (s.kind === "ended") latest.current.onEnded?.();
        },
        refused: setRefused,
      });
      sizes = sizeReporter((s) => conn.resize(s));
      sizes.report({ cols: term.cols, rows: term.rows });
      const onResize = term.onResize((s) => sizes?.report(s));
      const typing = new TextEncoder();
      const onData = term.onData((d) => conn.send(typing.encode(d)));
      dispose = () => {
        osc.dispose();
        termRef.current = null;
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
  }, [streamKey]);

  return (
    <div className={`terminal${offer && status.kind === "none" ? " offer" : ""}`} data-testid="terminal">
      <div className="term-status" data-testid="term-status">
        {lead}
        <span className="muted">{statusText(status, noneText)}</span>
        {refused && <span className="error"> · {refused}</span>}
        <TermCopy copy={copy} copyAll={copyAll} ready={pending !== null} takeReady={takeReady} copied={copied} />
      </div>
      {status.kind === "none" && (
        <div className="term-empty">
          <p className="muted" data-testid="placeholder">{placeholder}</p>
          {offer}
        </div>
      )}
      <div className="term-host" ref={host} data-testid="term-host" />
    </div>
  );
}
