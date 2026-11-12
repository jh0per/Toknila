// The network side of attaching files: upload each to the project's library (one shared routine for
// a card's designs and for a message to the assistant), then, for a card, one save of its designs.
// It decides nothing about files: the refusals are the ones of `refusedFiles` and the daemon's own
// words. A failed upload or save leaves the designs as they were.

import { get, patch } from "../api/http";
import { uploadDesign } from "../api/designs";
import type { Design, DesignList, TicketDetail } from "../api/types";
import type { AskAttachMsg, AskUploadEffect } from "./askAttach";
import { type AttachEffect, type AttachResult, pastedName } from "./attach";
import { looksLikeDesign, MAX_DESIGN_BYTES, refusedFiles } from "./designs";

const enc = encodeURIComponent;
const said = (e: unknown): string => (e instanceof Error ? e.message : String(e));

/// Uploads the files that can be designs to the project's library: the designs that made it, and
/// the sentences about the files that did not. A pasted picture is named `<prefix>-<n>.<ext>` (the
/// first number the library and `have` do not use).
async function uploadAll(project: string, files: File[], prefix: string, pasted: boolean, have: string[] = []) {
  const refused = refusedFiles(files);
  const fine = files.filter((f) => looksLikeDesign(f) && f.size <= MAX_DESIGN_BYTES);
  const problems = refused ? [refused] : [];
  const added: Design[] = [];
  if (fine.length === 0) return { added, problems };
  let taken = have;
  try {
    if (pasted) taken = [...have, ...(await get<DesignList>(`/projects/${enc(project)}/designs`)).designs.map((d) => d.name)];
  } catch (err) {
    return { added, problems: [said(err)] };
  }
  const used: string[] = [];
  for (const f of fine) {
    let file = f;
    if (pasted) {
      const name = pastedName(prefix, f.type, taken, used);
      used.push(name);
      file = new File([f], name, { type: f.type });
    }
    try {
      added.push(await uploadDesign(project, file));
    } catch (err) {
      problems.push(`${file.name}: ${said(err)}`);
    }
  }
  return { added, problems };
}

/// Runs the effect: the answer says what the card is now and what was refused.
export async function attachFiles(e: AttachEffect): Promise<AttachResult> {
  const { added, problems } = await uploadAll(e.project, e.files, e.card, Boolean(e.pasted));
  if (added.length === 0) return { type: "filesAttached", card: e.card, message: problems.join("; ") };
  try {
    const designs = [...e.current, ...added.map((d) => ({ design: d.id }))];
    const detail = await patch<TicketDetail>(`/tickets/${enc(e.card)}`, { designs });
    return { type: "filesAttached", card: e.card, detail, message: problems.join("; ") };
  } catch (err) {
    return { type: "filesAttached", card: e.card, message: [...problems, said(err)].join("; ") };
  }
}

/// Runs the effect of a message's attachments: the designs that joined the library, and the
/// sentence about the files that did not.
export async function uploadForAsk(e: AskUploadEffect): Promise<AskAttachMsg> {
  const { added, problems } = await uploadAll(e.project, e.files, e.prefix, Boolean(e.pasted), e.have);
  return { type: "askAttach", act: "done", where: e.where, designs: added, message: problems.join("; ") };
}
