// What a card's Details says first: a status sentence and the next step, in words. The status,
// the reason, the review turn and the queue place all come from the daemon; this only words them.

import type { Ticket, TicketDetail } from "../api/types";
import { reasonText } from "../format";
import { checkText } from "./checkPhase";

/// How the sentence is coloured: a person is needed (`needs`), things go on by themselves
/// (`going`), or nothing is asked (`quiet`, `done`).
export type Tone = "needs" | "going" | "quiet" | "done";

export interface StatusLine {
  tone: Tone;
  /// The sentence that leads the Details.
  text: string;
  /// The next step, when there is one; the buttons for it are the card's actions.
  next?: string;
  /// A link to a tab of the card that the next step is about (the design waits in Threads).
  tab?: { tab: "threads"; label: string };
}

/// What the card's chain and the queue say about when it starts.
export interface Pace {
  /// Place in line (1 is next) while the card waits for a free agent slot.
  queue: number | undefined;
  /// The chain starts its ready cards by itself.
  autostart: boolean;
  /// The clock the elapsed time of a running check is read against; the current time when absent.
  now?: number;
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
      return designLine(detail) ?? {
        tone: "going",
        text: "An agent is working on it",
        next: "Watch it in Terminal; it will ask here if it needs you.",
      };
    case "waiting":
      if (card.agent_paused) return pausedLine(card, pace.queue);
      return (card.status_reason?.status === "waiting" && card.status_reason.kind === "silent" ? designLine(detail) : undefined) ?? {
        tone: "needs",
        text: "Waiting for your answer",
        next: waitingText(card),
      };
    case "blocked":
      return { tone: "needs", text: `Blocked: ${blockedText(card)}`, next: can("unblock") ? "Fix what it says, then Unblock." : undefined };
    case "review": {
      const turn = detail?.review.awaiting;
      if (turn?.by === "agent") {
        return { tone: "going", text: "In review: the reviewer agent is checking the work", next: "You can read the agent's report below meanwhile." };
      }
      if (turn?.by === "check" && detail) return { tone: "going", text: checkText(detail.review, pace.now ?? Date.now()) };
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

/// The sentence of a card whose agent has a design on the table: waiting for the person's verdict
/// on a version, or working on the design after changes were asked for.
function designLine(detail: TicketDetail | undefined): StatusLine | undefined {
  if (detail?.design_waiting) {
    return {
      tone: "needs",
      text: "Waiting for you to approve the design",
      next: "Look at the design in Threads, then approve it or ask for changes. No code is written before you approve.",
      tab: { tab: "threads", label: "Open the design" },
    };
  }
  if (detail?.phase === "design") {
    return { tone: "going", text: "An agent is working on the design", next: "It will show it in Threads when it is ready for you to judge." };
  }
  return undefined;
}

/// The sentence of a card whose agent was paused to free its slot: what it waits for, that no
/// agent is running, and how it comes back.
function pausedLine(card: Ticket, queue: number | undefined): StatusLine {
  const r = card.status_reason;
  const asks = r?.status === "waiting" && r.kind !== "silent" ? `: ${r.text}` : "";
  if (card.agent_paused?.resume) {
    const place = queue === undefined ? "" : queue === 1 ? " (next in line)" : ` (#${queue} in line)`;
    return {
      tone: "going",
      text: `Answered: the agent resumes when a slot is free${place}`,
      next: "No agent is running now; the answer is waiting for it.",
    };
  }
  return {
    tone: "needs",
    text: `Waiting for your answer${asks}`,
    next: "Its agent is paused, not running, and holds no slot. Answer below and it resumes.",
  };
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
