import type { ChainDetail, RunTime, Size, Ticket, TicketDetail, UsageFigures } from "../api/types";
import { allows, subtaskProgress } from "../store/cardDetails";
import { type State, queuePosition } from "../store/state";
import { type CardContent, type CardLayout, type Foot, type Measure, autoCard, sizedCard } from "./cardSize";

/// What a card prints and what sits under its status line. The pill shows the fine status word
/// ("review", not "in progress · review"); its colour and dot say the group.
export function contentOf(
  t: Ticket, detail: TicketDetail | undefined, usage: UsageFigures | RunTime | undefined, queued: number | undefined,
): CardContent {
  const proposal = t.status === "proposed" && (allows(detail, "approve") || allows(detail, "reject"));
  const extras = Boolean(subtaskProgress(detail) || queued !== undefined || t.pr || t.done_out_of_order || usage || t.designs?.length);
  const foot: Foot = proposal ? "actions" : extras ? "extras" : "none";
  return { title: t.title, pill: t.status === "proposed" ? "awaiting approval" : t.status, key: t.key, foot, description: t.prompt };
}

/// The box of every card of the chain, by ticket id. `sizeOf` says the size a card has right now
/// (being dragged to it, just given, or saved); a card without one fits its content.
export function cardLayouts(
  chain: ChainDetail, state: State, sizeOf: (t: Ticket) => Size | null | undefined, m: Measure,
): Map<string, CardLayout> {
  return new Map(chain.tickets.map((t) => {
    const c = contentOf(t, state.details[t.key], chain.usage?.[t.key] ?? chain.run_time?.[t.key], queuePosition(state, t.id));
    const s = sizeOf(t);
    return [t.id, s ? sizedCard(c, s, m) : autoCard(c, m)];
  }));
}
