// Pictures and pdfs attached to a message for the assistant: each joins the project's library at
// once and shows as a chip under the box; sending names the chips in one last line. This keeps what
// the box draws (chips, an upload on its way, a refusal); the uploads are the effect in
// `askAttachEffects.ts`, which reuses the card page's upload code.

import type { Design } from "../api/types";
import type { Effect } from "./msg";
import type { State } from "./state";

/// What one box (a place of the planner's drafts: a project's key, or a card's draft key) holds.
export interface AskPlace {
  /// The designs attached to the message, in the library's names.
  chips: Design[];
  /// Files of the box are uploading: Send waits.
  busy: boolean;
  /// The sentence about the files that were refused or failed, until the next drop or send.
  refused: string;
}

export type AskAttachState = Record<string, AskPlace>;

export const initialAskAttach: AskAttachState = {};

const none: AskPlace = { chips: [], busy: false, refused: "" };

/// A message about attachments: the person adds files or takes one off, or an upload ended.
export type AskAttachMsg =
  /// `prefix` names a pasted picture: the card's key, or the project's key.
  | { type: "askAttach"; act: "add"; project: string; where: string; prefix: string; files: File[]; pasted?: boolean }
  | { type: "askAttach"; act: "remove"; where: string; design: string }
  /// The effect's answer: the designs that made it, and the sentence about the others.
  | { type: "askAttach"; act: "done"; where: string; designs: Design[]; message: string };

/// The effect: upload the files to the project's library.
export type AskUploadEffect = {
  type: "askUpload";
  project: string;
  where: string;
  prefix: string;
  files: File[];
  pasted?: boolean;
  /// The names the box already has, so a pasted picture does not take one of them.
  have: string[];
};

/// What a box holds now.
export const placeOf = (s: State, where: string): AskPlace => s.askAttach[where] ?? none;

/// The line that names the attachments, at the end of the message.
export const attachedLine = (chips: Design[]): string => (chips.length ? `Attached designs: ${chips.map((c) => c.name).join(", ")}` : "");

/// The message with its last line: the words, then the attachments (either may be missing).
export function withAttached(text: string, chips: Design[]): string {
  const line = attachedLine(chips);
  const words = text.trim();
  return words && line ? `${words}\n${line}` : words || line;
}

/// Whether the box can send: not while an upload runs, and only with words or an attachment.
export const canSend = (s: State, where: string, text: string): boolean => {
  const p = placeOf(s, where);
  return !p.busy && (text.trim() !== "" || p.chips.length > 0);
};

/// The box with its attachments forgotten (the message went).
export function clearAttached(s: State, where: string): State {
  const { [where]: _gone, ...rest } = s.askAttach;
  return { ...s, askAttach: rest };
}

/// The reducer of attaching files to a message.
export function reduceAskAttach(s: State, msg: AskAttachMsg): [State, Effect[]] {
  const put = (where: string, p: Partial<AskPlace>): State => ({ ...s, askAttach: { ...s.askAttach, [where]: { ...placeOf(s, where), ...p } } });
  switch (msg.act) {
    case "add": {
      const p = placeOf(s, msg.where);
      if (msg.files.length === 0 || p.busy) return [s, []];
      const { act: _act, type: _type, ...rest } = msg;
      return [put(msg.where, { busy: true, refused: "" }), [{ type: "askUpload", ...rest, have: p.chips.map((c) => c.name) }]];
    }
    case "remove":
      return [put(msg.where, { chips: placeOf(s, msg.where).chips.filter((c) => c.id !== msg.design) }), []];
    case "done": {
      const have = placeOf(s, msg.where).chips;
      const fresh = msg.designs.filter((d) => !have.some((c) => c.id === d.id));
      return [put(msg.where, { busy: false, refused: msg.message, chips: [...have, ...fresh] }), []];
    }
  }
}
