// Files dropped, pasted or picked on a card's Designs section: each joins the project's library and
// the card's designs in one step. This keeps what the page draws (uploading, a refusal); the
// uploads and the one save are the effect in `attachEffects.ts`.

import type { TicketDetail } from "../api/types";
import type { Effect } from "./msg";
import type { Ref } from "./designs";
import type { State } from "./state";

/// What the person did: `pasted` files come from the clipboard and are named after the card.
export type AttachIntent = { type: "attachFiles"; project: string; card: string; files: File[]; pasted?: boolean };

/// What the effect answered: the card as saved (none when nothing was attached) and the sentence
/// about the files that did not make it.
export type AttachResult = { type: "filesAttached"; card: string; detail?: TicketDetail; message: string };

/// The effect: the card's designs as they were, to add the new ones after.
export type AttachEffect = AttachIntent & { current: Ref[] };

export interface AttachState {
  /// The card whose files are on their way.
  card: string | undefined;
  /// The last drop's refusals, shown next to the section of the card it was for; empty when all was fine.
  message: string;
  of: string | undefined;
}

export const initialAttach: AttachState = { card: undefined, message: "", of: undefined };

/// The refusals of the last drop on `card`, as one sentence.
export const refusalOf = (s: State, card: string): string => (s.attach.card === undefined && s.attach.of === card ? s.attach.message : "");

/// Whether files of `card` are uploading.
export const uploading = (s: State, card: string): boolean => s.attach.card === card;

/// The name of a pasted image: the card's key, a counter and the extension of its type. The
/// counter is the first number whose name the library does not have (any letter case).
export function pastedName(card: string, type: string, taken: string[], used: string[] = []): string {
  const ext = type === "image/jpeg" ? "jpg" : type.replace("image/", "") || "png";
  const have = new Set([...taken, ...used].map((n) => n.toLowerCase()));
  let n = 1;
  while (have.has(`${card}-${n}.${ext}`.toLowerCase())) n += 1;
  return `${card}-${n}.${ext}`;
}

/// The reducer of attaching files.
export function reduceAttach(s: State, msg: AttachIntent | AttachResult): [State, Effect[]] {
  switch (msg.type) {
    case "attachFiles": {
      if (msg.files.length === 0 || s.attach.card !== undefined) return [s, []];
      const current = s.details[msg.card]?.ticket.designs ?? [];
      return [{ ...s, attach: { card: msg.card, message: "", of: msg.card } }, [{ ...msg, current }]];
    }
    case "filesAttached": {
      const details = msg.detail ? { ...s.details, [msg.card]: msg.detail } : s.details;
      const next = { ...s, details, attach: { card: undefined, message: msg.message, of: msg.card } };
      return [next, msg.detail ? [{ type: "loadChain", id: msg.detail.ticket.chain_id }] : []];
    }
  }
}
