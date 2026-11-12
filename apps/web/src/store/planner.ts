// A project's planner (the assistant): whether its process is alive, as the daemon answered, and
// the messages the person sends it. The planner edits the plan through its own tools, so the
// diagram follows the journal like any other change; this slice keeps the Start and Stop state,
// the refusal of the last command, and what is typed to the assistant before it is sent.

import type { PlannerState } from "../api/types";
import { lineFor, panelChip } from "./assistant";
import { type AskAttachMsg, clearAttached, placeOf, reduceAskAttach, withAttached } from "./askAttach";
import type { Effect } from "./msg";
import type { State } from "./state";

/// The built-in profile that launches Claude Code as a planner.
export const DEFAULT_PLANNER_PROFILE = "claude-code-planner";

/// The place the first-run "Describe your project" box keeps its text, until the project exists.
export const ONBOARDING_DRAFT = "onboarding";

export interface PlannerSlice {
  /// The daemon's answer per project key.
  byProject: Record<string, PlannerState>;
  /// The refusal of the last command (no repository, no `claude`), shown next to the buttons.
  error: string;
  /// The planner profile Start uses; a person can pick another planner profile.
  profile: string;
  /// What the person has typed to the assistant and not sent yet, per place: a project's key, or
  /// `ONBOARDING_DRAFT` before the first project exists.
  drafts: Record<string, string>;
  /// The project whose message is on its way: the planner is started, then the text is typed.
  sending: string | undefined;
  /// Projects whose planner this window has seen running: when it ends, its terminal stays on
  /// screen as the record of the session.
  seen: Record<string, boolean>;
}

export const initialPlanner: PlannerSlice = {
  byProject: {}, error: "", profile: DEFAULT_PLANNER_PROFILE, drafts: {}, sending: undefined, seen: {},
};

/// What the person did.
export type PlannerIntent =
  | { type: "startPlanner"; project: string }
  | { type: "stopPlanner"; project: string }
  /// The planner's terminal says its process ended: ask again whether it runs.
  | { type: "refreshPlanner"; project: string }
  | { type: "pickPlannerProfile"; profile: string }
  /// Typing in one of the boxes that talk to the assistant.
  | { type: "editPlannerDraft"; where: string; text: string }
  /// Send the project's draft to the assistant: start it if it is not running, then type the text.
  | { type: "askPlanner"; project: string }
  /// Deliver the card's unanswered message to the planner again (`thread` is the discussion's
  /// number, `text` the message), through the planner's input; nothing is stored twice.
  | { type: "nudgeCard"; project: string; key: string; thread: number; text: string }
  /// The same for a message of the chat that is about no card: its text is typed again.
  | { type: "nudgeChat"; project: string; text: string };

/// What the daemon answered.
export type PlannerResult =
  | { type: "plannerLoaded"; project: string; state: PlannerState }
  /// The text reached the planner's terminal; `draft` is the place whose text it was (the
  /// project's key unless it was said about a card).
  | { type: "plannerAsked"; project: string; draft?: string }
  | { type: "plannerFailed"; error: string };

export type PlannerMsg = PlannerIntent | PlannerResult | AskAttachMsg;

export type PlannerEffect = {
  type: "plannerCall";
  project: string;
  call: "load" | "start" | "stop" | "ask" | "discuss" | "nudge";
  /// `nudge`: the discussion's number.
  thread?: number;
  /// The profile a start uses.
  profile?: string;
  /// `discuss`: the card (key) the words are about, and the draft place they were typed in.
  card?: string;
  draft?: string;
  /// `ask`: the message, on one line (the terminal submits a line).
  text?: string;
  /// `ask`: the message as written, line breaks kept: it is stored in the conversation first.
  raw?: string;
  /// `ask`: what the stored message is about (a chain or the roadmap), when the chip said so.
  about?: { kind: "chain"; name: string } | { kind: "roadmap" };
  /// `ask`: the planner has to start first, so wait for its screen to settle before typing.
  wait?: boolean;
};

export const loadPlanner = (project: string): Effect => ({ type: "plannerCall", project, call: "load" });

/// A message as the terminal takes it: one line, since a line break would submit it half-way.
export const oneLine = (text: string): string => text.trim().replace(/\s*\n\s*/g, " ");

export function reducePlanner(s: State, msg: PlannerMsg): [State, Effect[]] {
  const set = (planner: Partial<PlannerSlice>): State => ({ ...s, planner: { ...s.planner, ...planner } });
  switch (msg.type) {
    case "editPlannerDraft":
      return [set({ drafts: { ...s.planner.drafts, [msg.where]: msg.text } }), []];
    case "askAttach":
      return reduceAskAttach(s, msg);
    case "askPlanner": {
      // The panel's one composer holds the attachments of every context (its place is the project).
      const raw = withAttached(s.planner.drafts[msg.project] ?? "", placeOf(s, msg.project).chips);
      if (!raw || s.planner.sending || placeOf(s, msg.project).busy) return [s, []];
      const wait = !(plannerOf(s, msg.project)?.running ?? false);
      const base = { type: "plannerCall", project: msg.project, profile: s.planner.profile, wait } as const;
      const chip = panelChip(s);
      // The chip says what the message is about: a card's goes to the card's discussion, a chain's
      // is stored with the chain, the roadmap's is typed with its prefix.
      const call: PlannerEffect =
        chip?.kind === "card" ? { ...base, call: "discuss", card: chip.key, draft: msg.project, text: raw }
        : chip ? { ...base, call: "ask", text: lineFor(chip, raw), raw, about: chip.kind === "chain" || chip.kind === "roadmap" ? chip : undefined }
        : { ...base, call: "ask", text: oneLine(raw), raw };
      return [set({ error: "", sending: msg.project }), [call]];
    }
    case "nudgeCard":
      return [
        set({ error: "" }),
        [{ type: "plannerCall", project: msg.project, call: "nudge", card: msg.key, thread: msg.thread, text: msg.text }],
      ];
    case "nudgeChat":
      return [set({ error: "" }), [{ type: "plannerCall", project: msg.project, call: "nudge", text: oneLine(msg.text) }]];
    case "plannerAsked":
      return [{ ...clearAttached(s, msg.draft ?? msg.project), planner: { ...s.planner, sending: undefined, drafts: { ...s.planner.drafts, [msg.draft ?? msg.project]: "" } } }, []];
    case "startPlanner":
      return [
        set({ error: "" }),
        [{ type: "plannerCall", project: msg.project, call: "start", profile: s.planner.profile }],
      ];
    case "stopPlanner":
      return [set({ error: "" }), [{ type: "plannerCall", project: msg.project, call: "stop" }]];
    case "pickPlannerProfile":
      return [set({ profile: msg.profile }), []];
    case "refreshPlanner":
      return [s, [loadPlanner(msg.project)]];
    case "plannerLoaded": {
      const seen = msg.state.running ? { ...s.planner.seen, [msg.project]: true } : s.planner.seen;
      // The assistant starting or stopping changes what runs in the project.
      const changed = (plannerOf(s, msg.project)?.running ?? false) !== msg.state.running;
      const fx: Effect[] = changed ? [{ type: "loadAgents" }] : [];
      return [set({ byProject: { ...s.planner.byProject, [msg.project]: msg.state }, seen }), fx];
    }
    case "plannerFailed":
      return [set({ error: msg.error, sending: undefined }), []];
  }
}

/// The planner of a project as last answered; not running until the daemon says so.
export const plannerOf = (s: State, project: string): PlannerState | undefined => s.planner.byProject[project];

/// What is typed to the assistant in a place, not sent yet.
export const draftOf = (s: State, where: string): string => s.planner.drafts[where] ?? "";

/// Whether the assistant's terminal belongs on screen: it runs, or this window saw it run.
export const showsTerminal = (s: State, project: string): boolean =>
  (plannerOf(s, project)?.running ?? false) || Boolean(s.planner.seen[project]);
