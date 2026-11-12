// The network side of attaching files to a card: upload each to the project's library, then one
// save of the card's designs. It decides nothing about files: the refusals are the ones of
// `refusedFiles` and the daemon's own words. A failed upload or save leaves the designs as they were.

import { get, patch } from "../api/http";
import { uploadDesign } from "../api/designs";
import type { DesignList, TicketDetail } from "../api/types";
import { type AttachEffect, type AttachResult, pastedName } from "./attach";
import { type Ref, looksLikeDesign, MAX_DESIGN_BYTES, refusedFiles } from "./designs";

const enc = encodeURIComponent;
const said = (e: unknown): string => (e instanceof Error ? e.message : String(e));

/// Runs the effect: the answer says what the card is now and what was refused.
export async function attachFiles(e: AttachEffect): Promise<AttachResult> {
  const project = e.project;
  const refused = refusedFiles(e.files);
  const fine = e.files.filter((f) => looksLikeDesign(f) && f.size <= MAX_DESIGN_BYTES);
  const problems = refused ? [refused] : [];
  const added: Ref[] = [];
  if (fine.length > 0) {
    let taken: string[] = [];
    try {
      if (e.pasted) taken = (await get<DesignList>(`/projects/${enc(project)}/designs`)).designs.map((d) => d.name);
    } catch (err) {
      return { type: "filesAttached", card: e.card, message: said(err) };
    }
    const used: string[] = [];
    for (const f of fine) {
      let file = f;
      if (e.pasted) {
        const name = pastedName(e.card, f.type, taken, used);
        used.push(name);
        file = new File([f], name, { type: f.type });
      }
      try {
        const d = await uploadDesign(project, file);
        added.push({ design: d.id });
      } catch (err) {
        problems.push(`${file.name}: ${said(err)}`);
      }
    }
  }
  if (added.length === 0) return { type: "filesAttached", card: e.card, message: problems.join("; ") };
  try {
    const detail = await patch<TicketDetail>(`/tickets/${enc(e.card)}`, { designs: [...e.current, ...added] });
    return { type: "filesAttached", card: e.card, detail, message: problems.join("; ") };
  } catch (err) {
    return { type: "filesAttached", card: e.card, message: [...problems, said(err)].join("; ") };
  }
}
