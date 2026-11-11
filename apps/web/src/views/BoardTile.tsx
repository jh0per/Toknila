import type { PointerEvent as ReactPointerEvent } from "react";
import { token } from "../api/http";
import { type Item, type Tile, type Use, isPicture, labelSlots, regionOf } from "../store/designBoard";
import { kindLabel, placeholder, regionStyle, thumbUrl } from "../store/designs";
import type { Pin } from "../store/pins";
import { PinMarkers } from "./PinOverlay";
import type { RefDrag } from "./useRefDrag";

/// Width of a card's label on a region, in pixels at 100%: labels that start at the same corner
/// are set side by side by this much.
const LABEL_W = 78;

/// What the board needs to tell about a card: open it, start moving one of its references.
export interface TileActions {
  open: (card: string) => void;
  grab: (e: ReactPointerEvent, card: string, use: Use, design: string) => void;
}

/// One design on the board: its name, its picture with every card's region outlined in the card's
/// status colour, the pins, and on each region the card's label (opens the card) and a handle
/// (drag it onto another card to move the reference there). A design that cannot be shown as a
/// picture is a note with the cards that use it.
export function BoardTile({ project, item, tile, pins, activePin, onPin, onAspect, current, drag, actions }: {
  project: string;
  item: Item;
  tile: Tile;
  pins: Pin[];
  activePin: string | undefined;
  onPin: (id: string) => void;
  onAspect: (design: string, aspect: number) => void;
  /// The key of the card that is open, which gets the heavier outline.
  current: string | undefined;
  drag: RefDrag | undefined;
  actions: TileActions;
}) {
  const d = item.design;
  const slots = labelSlots(item.uses);
  const style = { left: tile.x, top: tile.y, width: tile.w };
  return (
    <figure className="board-tile" data-testid="board-tile" data-design={item.id} data-name={item.name} style={style}>
      <figcaption className="board-caption">
        <strong data-testid="board-tile-name">{item.name}</strong>
        {d && <span className="muted">{kindLabel(d)}</span>}
        <span className="muted">{item.uses.length} {item.uses.length === 1 ? "card" : "cards"}</span>
      </figcaption>
      {d && isPicture(d) ? (
        <div className="board-stage" data-testid="board-stage" style={{ height: tile.h }}>
          <img
            src={thumbUrl(project, d, token())} alt={item.name} draggable={false} data-testid="board-image"
            onLoad={(e) => {
              const { naturalWidth: w, naturalHeight: h } = e.currentTarget;
              if (w > 0 && h > 0) onAspect(item.id, w / h);
            }}
          />
          {item.uses.map((u, i) => (
            <span
              key={`${u.card}-${i}`} className={`board-region s-${u.status}${current === u.card ? " current" : ""}${drag?.over === u.card ? " over" : ""}`}
              data-testid="board-region" data-card={u.card} data-drop-card={u.card} data-status={u.status} style={regionStyle(regionOf(u))}
            >
              <span className="board-tag" style={{ left: slots[i] * LABEL_W }}>
                <button type="button" className="board-label" data-testid="board-label" title={`Open ${u.card}`} onClick={() => actions.open(u.card)}>
                  {u.card}
                </button>
                <span
                  className="board-handle" data-testid="board-handle" role="button" aria-label={`Move ${u.card}'s reference to another card`}
                  title="Drag onto another card to move this reference there" onPointerDown={(e) => actions.grab(e, u.card, u, item.id)}
                />
              </span>
            </span>
          ))}
          <PinMarkers pins={pins} active={activePin} onSelect={onPin} />
        </div>
      ) : (
        <div className="board-note" data-testid="board-note" style={{ height: tile.h }}>
          <p className="muted">
            {d ? (d.kind === "pdf" ? "This PDF has no first page picture on this machine. Open a card to read it." : placeholder(d)) : "This design is no longer in the project's library."}
          </p>
          <span className="board-note-cards">
            {item.uses.map((u, i) => (
              <button
                key={`${u.card}-${i}`} type="button" className={`board-chip s-${u.status}`} data-testid="board-note-card" data-drop-card={u.card}
                onClick={() => actions.open(u.card)}
              >
                {u.card}
              </button>
            ))}
          </span>
        </div>
      )}
    </figure>
  );
}
