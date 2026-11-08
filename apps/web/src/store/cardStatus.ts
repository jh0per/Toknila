// What a card's Details says first: a status sentence and the next step, in words. The status,
// the reason, the review turn and the queue place all come from the daemon; this only words them.

import type { Ticket, TicketDetail } from "../api/types";
import { reasonText } from "../format";

/// How the sentence is coloured: a person is needed (`needs`), things go on by themselves
/// (`going`), or nothing is asked (`quiet`, `done`).
export type Tone = "needs" | "going" | "quiet" | "done";

export interface StatusLine {
  tone: Tone;
  /// The sentence that leads the Details.
  text: string;
  /// The next step, when there is one; the buttons for it are the card's actions.
  next?: string;
}

/// What the card's chain and the queue say about when it starts.
export interface Pace {
  /// Place in line (1 is next) while the card waits for a free agent slot.
  queue: number | undefined;
  /// The chain starts its ready cards by itself.
  autostart: boolean;
}

/// The status sentence and the next step of a card.
export function statusLine(card: Ticket, detail: TicketDetail | undefined, pace: Pace): StatusLine {
  const can = (a: string) => (detail?.allowed_actions ?? []).some((x) => x === a);
  switch (card.status) {
    case "proposed":
      return { tone: "needs", text: "Proposed: waiting for your approval", next: "Approve it to add it to the plan, or Reject it." };
    case "backlog":
      return card.paused
        ? { tone: "quiet", text: "Paused: it will not start until you resume it", next: can("resume") ? "Resume it when you want it to go on." : undefined }
        : {
            tone: "quiet",
            text: "Not started: it waits for the cards before it",
            next: can("start") || can("start_anyway") ? "You can also start it now." : undefined,
          };
    case "ready":
      if (pace.queue !== undefined) {
        return { tone: "going", text: `Ready: starts when a slot is free (${pace.queue === 1 ? "next in line" : `#${pace.queue} in line`})` };
      }
      return pace.autostart
        ? { tone: "going", text: "Ready: starts when a slot is free" }
        : { tone: "needs", text: "Ready: start it when you want", next: "This chain does not start cards automatically; press Start." };
    case "running":
      return { tone: "going", text: "An agent is working on it", next: "Watch it in Terminal; it will ask here if it needs you." };
    case "waiting":
      return { tone: "needs", text: "Waiting for your answer", next: waitingText(card) };
    case "blocked":
      return { tone: "needs", text: `Blocked: ${blockedText(card)}`, next: can("unblock") ? "Fix what it says, then Unblock." : undefined };
    case "review": {
      const turn = detail?.review.awaiting;
      if (turn?.by === "agent") {
        return { tone: "going", text: "In review: the reviewer agent is checking the work", next: "You can read the agent's report below meanwhile." };
      }
      if (turn?.by === "merging") return { tone: "going", text: "Approved: merging into the main branch" };
      return { tone: "needs", text: "In review: read the agent's report below, then Accept or Return" };
    }
    case "merged":
      return { tone: "done", text: "Done" };
    case "released":
      return { tone: "done", text: "Done and released" };
    case "cancelled": {
      const why = card.status_reason?.status === "cancelled" ? card.status_reason.text : "";
      return { tone: "quiet", text: why ? `Cancelled: ${why}` : "Cancelled", next: can("reopen") ? "Reopen it to bring it back." : undefined };
    }
  }
}

/// What a waiting card asks, in a sentence.
function waitingText(card: Ticket): string | undefined {
  const r = card.status_reason;
  if (r?.status !== "waiting") return undefined;
  if (r.kind === "silent") return "The agent went silent. Look at its Terminal, or write to it below.";
  return r.text;
}

/// Why a blocked card is blocked, without the word "blocked" in front.
function blockedText(card: Ticket): string {
  const text = reasonText(card.status_reason);
  return text.replace(/^blocked: /, "") || "it cannot go on by itself";
}
