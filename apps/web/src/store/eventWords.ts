// The journal in words: one sentence per event, for a card's Events tab. The raw payload stays
// behind a disclosure there. Every kind the daemon journals has words here (a test reads the
// daemon's list of kinds), and an unknown one still prints, as "entity kind".

import type { Actor, EventRecord } from "../api/types";

/// The journal's own names for what happened, as (entity, kind) pairs, with a sentence for each.
/// The payload is read field by field and may lack any of them.
type Words = (p: Fields) => string;
type Fields = Record<string, any>;

const str = (v: unknown): string => (typeof v === "string" ? v : "");
const status = (v: unknown): string => str(v).replace(/_/g, " ");

const ANSWER: Record<string, string> = {
  allow: "Allowed", deny: "Turned down", text: "Typed an answer on its screen", enter: "Pressed Enter",
  one: "Chose 1", two: "Chose 2", up: "Pressed the up arrow", down: "Pressed the down arrow", escape: "Pressed Esc",
};

/// "an instruction", "a report", "a comment".
const aKind = (kind: unknown): string => {
  const k = str(kind) || "comment";
  return `${/^[aeiou]/.test(k) ? "an" : "a"} ${k}`;
};

/// What a person's answer to a question was, in words.
export const answerWords = (answer: unknown): string => ANSWER[str(answer)] ?? str(answer);

const RUN_ROLE: Record<string, string> = { worker: "The agent", reviewer: "The reviewer", planner: "The assistant" };

const turn = (p: Fields): string => (p.role === "reviewer" ? "The reviewer" : "The agent");

const BLOCKED: Record<string, string> = {
  manual: "blocked by hand", crashed: "crashed", terminated: "the agent was terminated from outside", dependency_reopened: "a card it depends on was reopened",
  merge_conflict: "the merge conflicts", merge_failed: "the merge failed", ci_failed: "the checks failed",
  pr_closed: "the pull request was closed", runner_lost: "the agent was lost", ship_failed: "its workspace failed",
};

/// Why a card moved to its new status, when the journal says.
function becauseOf(reason: Fields | null | undefined): string {
  if (!reason) return "";
  switch (reason.status) {
    case "waiting":
      return reason.kind === "silent" ? " (the agent went silent)" : reason.text ? ` (${str(reason.kind)}: ${str(reason.text)})` : "";
    case "cancelled":
      return reason.text ? ` (${str(reason.text)})` : "";
    case "blocked": {
      const why = BLOCKED[str(reason.reason?.kind)];
      const text = str(reason.reason?.text) || str(reason.reason?.detail);
      return why ? ` (${why}${text ? `: ${text}` : ""})` : text ? ` (${text})` : "";
    }
    default:
      return "";
  }
}

const TABLE: Record<string, Words> = {
  "project.created": (p) => `Project ${str(p.project?.key)} added`,
  "project.updated": (p) => `Settings of project ${str(p.project?.key)} changed`,
  "project.deleted": (p) => `Project ${str(p.project?.key)} deleted`,
  "chain.created": (p) => `Chain ${str(p.chain?.name)} created`,
  "chain.updated": (p) => `Settings of chain ${str(p.chain?.name)} changed`,
  "chain.deleted": (p) => `Chain ${str(p.chain?.name)} deleted`,
  "chain.edge_added": () => "A card now waits for another",
  "chain.edge_removed": () => "A card no longer waits for another",
  "chain.edge_moved": () => "A dependency moved to another card",
  "ticket.design_moved": (p) => `A design reference moved here from ${str(p.from)}`,
  "ticket.created": () => "Card created",
  "ticket.status_changed": (p) => `Moved from ${status(p.from)} to ${status(p.to)}${becauseOf(p.reason)}`,
  "ticket.updated": () => "Card edited",
  "ticket.proposed": () => "Proposed as a new card, waiting for approval",
  "ticket.approved": () => "Approved and added to the plan",
  "ticket.rejected": (p) => `Rejected${p.reason ? `: ${str(p.reason)}` : ""}`,
  "ticket.ship_created": () => "Workspace made for the agent",
  "ticket.ship_removed": () => "Workspace removed (its branch stays)",
  "ticket.ship_remove_failed": (p) => `The workspace could not be removed${p.detail ? `: ${str(p.detail)}` : ""}`,
  "ticket.runner_started": (p) => `${RUN_ROLE[str(p.run?.role)] ?? "The agent"} started`,
  "ticket.runner_exited": (p) => `${RUN_ROLE[str(p.run?.role)] ?? "The agent"} stopped`,
  "ticket.question_raised": (p) =>
    p.kind === "permission" ? `${turn(p)} asks for permission`
    : p.kind === "screen" ? `${turn(p)} waits for you on its screen`
    : `${turn(p)} asks a question`,
  "ticket.question_cleared": (p) => `${turn(p)} went on without an answer`,
  "ticket.question_answered": (p) => `Answered: ${answerWords(p.answer)}`,
  "release.created": (p) => `Milestone ${str(p.release?.version)} created`,
  "release.state_changed": (p) => `Milestone ${str(p.version)} moved from ${status(p.from)} to ${status(p.to)}`,
  "comment.added": (p) => `Wrote ${aKind(p.comment?.kind)}: ${str(p.comment?.body?.gist)}`,
  "comment.delivered": (p) => `The agent received ${aKind(p.comment?.kind)}: ${str(p.comment?.body?.gist)}`,
  "subtask.added": (p) => `Subtask added: ${str(p.subtask?.text)}`,
  "subtask.updated": (p) => `Subtask ${p.subtask?.done ? "done" : "reopened"}: ${str(p.subtask?.text)}`,
  "thread.opened": (p) => `Thread opened: ${str(p.thread?.title)}`,
  "thread.message_posted": () => "Message in a thread",
  "thread.version_posted": (p) => (typeof p.number === "number" ? `Version ${p.number} of a thread posted` : "New version of a thread posted"),
  "thread.approved": (p) => `Thread approved: ${str(p.thread?.title)}`,
  "thread.reopened": (p) => `Thread reopened: ${str(p.thread?.title)}`,
  "thread.withdrawn": (p) => `Thread withdrawn: ${str(p.thread?.title)}`,
  "design_pin.asked": (p) => `The assistant pinned a question on ${str(p.pin?.design_name)}: ${str(p.pin?.text)}`,
  "design_pin.answered": (p) => `Answered a question on ${str(p.pin?.design_name)}: ${str(p.pin?.answer)}`,
  "window.show_card": (p) => `The assistant opened ${str(p.ticket) || "a card"}`,
  "window.suggest": (p) => `The assistant says the next step: ${str(p.suggestion?.text)}`,
  "window.suggestion_cleared": () => "The assistant's next step is done",
};

/// Every (entity, kind) the journal has words for.
export const WORDED: readonly string[] = Object.keys(TABLE);

/// Who did it, as a person would say it.
export const WHO: Record<Actor, string> = { human: "You", agent: "The agent", system: "Toknila" };

/// One event as a sentence; an event this page has no words for reads "entity kind".
export function eventWords(e: Pick<EventRecord, "entity" | "kind" | "payload">): string {
  const words = TABLE[`${e.entity}.${e.kind}`];
  const p = (e.payload && typeof e.payload === "object" ? e.payload : {}) as Fields;
  return words ? words(p).trim() : `${e.entity} ${e.kind.replace(/_/g, " ")}`;
}
