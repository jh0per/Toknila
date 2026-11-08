import type { CSSProperties, MouseEvent } from "react";
import type { Release, Ticket, TicketDetail, UsageFigures } from "../api/types";
import { outOfOrderText } from "../format";
import { nodeKey } from "../layout";
import { allows, subtaskProgress } from "../store/cardDetails";
import { statusClass, statusGroup, statusText } from "../store/groups";
import { showAgentName } from "../store/profiles";
import { PrChip } from "./PrChip";
import { UsageChip } from "./UsageChip";

/// A card of the diagram. It reads colour from its fine status (`s-<status>`), shows how far its
/// subtasks are when it has any, and a proposed card carries its own Approve and Reject, the same
/// actions the Inbox offers, when the daemon allows them. The agent's runner shows only when it is
/// not the project's default; the usage figures show here and nowhere else on the page.
export function TicketNode({ ticket: t, href, style, selected, over, linking, detail, usage, queued, defaultProfile, onHover, handlers }: {
  ticket: Ticket;
  href: string;
  style: CSSProperties;
  selected: boolean;
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
    focus: () => void;
    menu: (e: MouseEvent) => void;
    approve: () => void;
    reject: (at: { x: number; y: number }) => void;
  };
}) {
  const progress = subtaskProgress(detail);
  const classes = [
    "node", "ticket", `g-${statusGroup(t.status)}`, statusClass(t.status),
    selected ? "sel" : "", over ? "over" : "", linking ? "linkable" : "",
  ].filter(Boolean).join(" ");
  const proposal = t.status === "proposed" && (allows(detail, "approve") || allows(detail, "reject"));
  const extras = Boolean(progress || queued !== undefined || t.pr || t.done_out_of_order || usage);
  const act = (f: () => void) => (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    f();
  };
  return (
    <a
      className={classes} data-key={t.key} data-node={nodeKey({ kind: "ticket", id: t.id })} href={href} draggable={false} style={style}
      onClick={act(handlers.activate)}
      onPointerEnter={() => onHover(true)}
      onPointerLeave={() => onHover(false)}
      // A focused card's allowed actions are read, so S can start it.
      onFocus={handlers.focus}
      onContextMenu={handlers.menu}
    >
      <span className="row">
        <span className="title">{t.title}</span>
        {t.profile && showAgentName(t.profile, defaultProfile, import.meta.env.DEV) && <span className="runner">{t.profile}</span>}
      </span>
      <span className="row">
        <span className="st" data-testid="status">
          {t.status === "running" && <span className="dot pulse" data-testid="live" />}
          {t.status === "proposed" ? "awaiting approval" : statusText(t.status)}
        </span>
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
    </a>
  );
}

/// A release milestone: a dashed node at the end of its cards.
export function ReleaseNode({ release: r, nodeKey, style, over }: { release: Release; nodeKey: string; style: CSSProperties; over: boolean }) {
  return (
    <div className={`node release${over ? " over" : ""}`} data-node={nodeKey} data-version={r.version} style={style}>
      <b className="key">◆ {r.version}</b>
      <span className="muted">{r.channel} · {r.start_mode}</span>
      <small className="muted" data-testid="release-state">{r.state}</small>
    </div>
  );
}
