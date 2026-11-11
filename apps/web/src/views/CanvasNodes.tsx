import { type CSSProperties, type MouseEvent, type PointerEvent, useState } from "react";
import type { ChainSummary, Release, Ticket, TicketDetail, UsageFigures } from "../api/types";
import { outOfOrderText } from "../format";
import { nodeKey } from "../layout";
import { allows, subtaskProgress } from "../store/cardDetails";
import { statusClass, statusGroup } from "../store/groups";
import { finishText, finishView } from "../store/phase";
import { showAgentName } from "../store/profiles";
import type { CardLayout } from "./cardSize";
import { Markdown } from "./Markdown";
import { NodeThumb } from "./NodeThumb";
import { PrChip } from "./PrChip";
import { UsageChip } from "./UsageChip";

/// A card of the diagram. It reads colour from its fine status (`s-<status>`), shows how far its
/// subtasks are when it has any, and a proposed card carries its own Approve and Reject, the same
/// actions the Inbox offers, when the daemon allows them. The agent's runner shows only when it is
/// not the project's default; the usage figures show here and nowhere else on the page.
export function TicketNode({ ticket: t, projectKey, layout, href, style, selected, moving, over, linking, detail, usage, queued, defaultProfile, onHover, handlers }: {
  ticket: Ticket;
  /// The key of the card's project: its designs are served under it.
  projectKey: string;
  /// The title as wrapped for the card's width (`cardSize.ts`).
  layout: CardLayout;
  href: string;
  style: CSSProperties;
  selected: boolean;
  /// The card is being dragged to a new place.
  moving: boolean;
  /// A line being dragged is over this card.
  over: boolean;
  /// The card can be picked as the one a card waits for ("Add dependency").
  linking: boolean;
  detail: TicketDetail | undefined;
  usage: UsageFigures | undefined;
  /// Place in line while it waits for a free agent slot.
  queued: number | undefined;
  /// The project's default profile: the runner chip stays away when the card uses it.
  defaultProfile: string | undefined;
  /// The pointer is on the card (its connector shows).
  onHover: (on: boolean) => void;
  handlers: {
    activate: () => void;
    /// The pointer went down on the card: it may start dragging it.
    press: (e: PointerEvent) => void;
    /// The pointer went down on the resize grip.
    grip: (e: PointerEvent) => void;
    /// Back to the size the content makes.
    fit: () => void;
    focus: () => void;
    menu: (e: MouseEvent) => void;
    approve: () => void;
    reject: (at: { x: number; y: number }) => void;
  };
}) {
  const progress = subtaskProgress(detail);
  const classes = [
    "node", "ticket", `g-${statusGroup(t.status)}`, statusClass(t.status),
    selected ? "sel" : "", moving ? "moving" : "", over ? "over" : "", linking ? "linkable" : "",
  ].filter(Boolean).join(" ");
  const proposal = t.status === "proposed" && (allows(detail, "approve") || allows(detail, "reject"));
  const extras = Boolean(progress || queued !== undefined || t.pr || t.done_out_of_order || usage || t.designs?.length);
  const act = (f: () => void) => (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    f();
  };
  return (
    <a
      className={classes} data-key={t.key} data-node={nodeKey({ kind: "ticket", id: t.id })} href={href} draggable={false} style={style}
      onClick={act(handlers.activate)}
      onPointerDown={handlers.press}
      onPointerEnter={() => onHover(true)}
      onPointerLeave={() => onHover(false)}
      // A focused card's allowed actions are read, so S can start it.
      onFocus={handlers.focus}
      onContextMenu={handlers.menu}
    >
      <span className="title" title={t.title} data-testid="title">
        {layout.lines.map((line, i) => <span className="tl" key={i}>{line}</span>)}
      </span>
      {layout.desc && <span className="desc" data-testid="desc">{layout.desc}</span>}
      <span className="row">
        <span className="st" data-testid="status">
          <span className={`dot${t.status === "running" ? " pulse" : ""}`} data-testid={t.status === "running" ? "live" : undefined} />
          {t.status === "proposed" ? "awaiting approval" : t.status}
        </span>
        {t.profile && showAgentName(t.profile, defaultProfile, import.meta.env.DEV) && <span className="runner">{t.profile}</span>}
        <span className="key">{t.key}</span>
      </span>
      {proposal ? (
        <span className="foot acts" data-testid="proposal-actions">
          {allows(detail, "approve") && (
            <button type="button" className="btn primary" data-testid="card-approve" onClick={act(handlers.approve)}>Approve</button>
          )}
          {allows(detail, "reject") && (
            <button
              type="button" className="btn danger" data-testid="card-reject"
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                handlers.reject({ x: e.clientX, y: e.clientY });
              }}
            >
              Reject
            </button>
          )}
        </span>
      ) : extras && (
        <span className="foot">
          {t.designs && t.designs.length > 0 && <NodeThumb project={projectKey} designs={t.designs} />}
          {progress && (
            <span className="sub" data-testid="subtasks" title={`${progress.done} of ${progress.total} subtasks done`}>
              <span className="sub-bar"><i style={{ width: `${(100 * progress.done) / progress.total}%` }} /></span>
              <b>{progress.done}/{progress.total}</b>
            </span>
          )}
          {queued !== undefined && (
            <span className="mark queued" data-testid="queued" title="Waiting for a free agent slot">queued #{queued}</span>
          )}
          {t.pr && <PrChip pr={t.pr} />}
          {t.done_out_of_order && <span className="mark" title={outOfOrderText(t.done_out_of_order)}>out of order</span>}
          <UsageChip figures={usage} />
        </span>
      )}
      <span
        className="grip" data-testid="grip" title="Drag to resize, double-click to fit the text"
        onPointerDown={handlers.grip}
        onClick={(e) => { e.preventDefault(); e.stopPropagation(); }}
        onDoubleClick={act(handlers.fit)}
      />
    </a>
  );
}

/// A release milestone: a dashed node at the end of its cards. The chain's last release is the
/// end of the diagram (`end`).
export function ReleaseNode({ release: r, nodeKey, style, over, end = false, onRelease }: {
  release: Release;
  nodeKey: string;
  style: CSSProperties;
  over: boolean;
  end?: boolean;
  /// Set when every card of the chain is merged: the node offers Release.
  onRelease?: () => void;
}) {
  return (
    <div className={`node release${over ? " over" : ""}`} data-node={nodeKey} data-version={r.version} data-end={end || undefined} style={style}>
      <b className="key">◆ {r.version}</b>
      <span className="muted">{r.channel} · {r.start_mode}</span>
      <small className="muted" data-testid="release-state">{r.state}{r.tag ? ` · ${r.tag}` : ""}</small>
      {onRelease && <ReleaseButton onClick={onRelease} />}
    </div>
  );
}

/// The Release button of an end node; a press on it is not a drag of the diagram.
function ReleaseButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      type="button" className="btn primary small" data-testid="release-button"
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => { e.stopPropagation(); onClick(); }}
    >
      Release
    </button>
  );
}

/// Where the diagram starts: the chain's epic. The name is the epic's title, the goal (markdown) its
/// description, a few lines of it with the rest behind "more". Opening it edits the epic.
export function StartNode({ name, goal, style, selected, onOpen }: {
  name: string;
  goal: string;
  style: CSSProperties;
  selected: boolean;
  onOpen: () => void;
}) {
  const [more, setMore] = useState(false);
  const long = goal.split("\n").filter((l) => l.trim()).length > 3 || goal.length > 140;
  return (
    <div
      className={`node bookend epic${selected ? " sel" : ""}${more ? " more" : ""}`} data-testid="start-node" style={style} role="button" tabIndex={0}
      onClick={(e) => { e.stopPropagation(); onOpen(); }}
      onKeyDown={(e) => { if (e.key === "Enter") onOpen(); }}
    >
      <b className="key" data-testid="epic-title" title={name}>{name}</b>
      <span className="muted goal" data-testid="epic-goal" title={more ? undefined : goal}>
        {goal.trim() ? <Markdown text={goal} /> : "No goal yet"}
      </span>
      {long && (
        <button
          type="button" className="link-btn more-btn" data-testid="epic-more"
          onPointerDown={(e) => e.stopPropagation()}
          onClick={(e) => { e.stopPropagation(); setMore(!more); }}
        >
          {more ? "less ▴" : "more ▾"}
        </button>
      )}
    </div>
  );
}

/// Where the diagram ends: the chain's release. With a version it reads `Release v0.3.0` and its
/// state (planned, ready to release, released with the date and the tag); without one, the
/// phase's words. It is the chain's end release when it has one (`release`, a milestone node).
export function FinishNode({ style, summary, release, nodeKey, over = false, selected = false, onRelease, onOpen }: {
  style: CSSProperties;
  summary: ChainSummary;
  release?: Release;
  nodeKey?: string;
  over?: boolean;
  selected?: boolean;
  onRelease?: () => void;
  onOpen?: () => void;
}) {
  const view = finishView(summary);
  const text = finishText(summary);
  return (
    <div
      className={`node bookend finish${over ? " over" : ""}${selected ? " sel" : ""}`} data-testid="end-node" data-end data-phase={summary.phase}
      data-state={view?.state} data-node={nodeKey} data-version={release?.version} style={style}
      onClick={onOpen && ((e) => { e.stopPropagation(); onOpen(); })}
    >
      {view ? (
        <>
          <b className="key" data-testid="finish-title">{view.title}</b>
          <span className={`finish-state s-${view.state}`} data-testid="finish-state">{view.stateText}</span>
          {view.text && <span className="muted" data-testid="end-node-text">{view.text}</span>}
        </>
      ) : (
        <>
          <b className="key">{summary.phase === "finished" ? "Finished" : "Finish"}</b>
          <span className="muted" data-testid="end-node-text">{text}</span>
        </>
      )}
      {onRelease && <ReleaseButton onClick={onRelease} />}
    </div>
  );
}
