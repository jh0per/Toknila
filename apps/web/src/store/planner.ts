// A project's planner (the assistant): whether its process is alive, as the daemon answered, and
// the messages the person sends it. The planner edits the plan through its own tools, so the
// diagram follows the journal like any other change; this slice keeps the Start and Stop state,
// the refusal of the last command, and what is typed to the assistant before it is sent. Its turn
// (`working`, `step`, `since`) is the daemon's: a `planner.turn` journal entry starts or ends it at
// once, and while it is on and shown the window asks for the step (`syncPoll`, the timer in
// `plannerEffects.ts`); what the chat and the boxes make of it is `plannerTurn.ts`.

import type { ConversationMessage, PlannerState } from "../api/types";
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
  /// A link asked for the assistant's Terminal tab; the screen opens it and says it did.
  terminalWanted: boolean;
  /// Ids of messages the person sent again after the assistant stopped: the copy is the one
  /// awaited, so these no longer count as unread (this window's memory, not the daemon's).
  resent: string[];
  /// How many chats and ask boxes of each project are on the screen: while one is and the
  /// project's turn is on, the window asks for the assistant's step.
  shown: Record<string, number>;
  /// The projects whose step the window asks for now, every `STEP_POLL_MS` (the effect's timers).
  polling: string[];
  /// The ask box each project's last message was sent from (`chain:<name>`, `roadmap`); none when
  /// it was sent from the chat. That box shows what was sent until the assistant answers.
  from: Record<string, string>;
}

export const initialPlanner: PlannerSlice = {
  byProject: {}, error: "", profile: DEFAULT_PLANNER_PROFILE, drafts: {}, sending: undefined, seen: {}, terminalWanted: false, resent: [],
  shown: {}, polling: [], from: {},
};

/// How often the window asks the daemon for the assistant's step while its turn is on and on the screen.
export const STEP_POLL_MS = 2_000;

/// What the person did.
export type PlannerIntent =
  | { type: "startPlanner"; project: string }
  | { type: "stopPlanner"; project: string }
  /// The planner's terminal says its process ended: ask again whether it runs.
  | { type: "refreshPlanner"; project: string }
  | { type: "pickPlannerProfile"; profile: string }
  /// A link to the assistant's Terminal tab was followed; `plannerTerminalShown` answers it.
  | { type: "wantPlannerTerminal" }
  | { type: "plannerTerminalShown" }
  /// Typing in one of the boxes that talk to the assistant.
  | { type: "editPlannerDraft"; where: string; text: string }
  /// Send the project's draft to the assistant: start it if it is not running, then type the text.
  /// `from` is the ask box it was sent from (none for the chat).
  | { type: "askPlanner"; project: string; from?: string }
  /// A chat or an ask box of the project came on the screen (`on`) or left it.
  | { type: "plannerShown"; project: string; on: boolean }
  /// A question asked in a story's chat: kept in the chat by the daemon, then typed to the
  /// assistant with the story's record. Starts the assistant when it is not running.
  | { type: "askStory"; project: string; key: string; text: string }
  /// Deliver the card's unanswered message to the planner again (`thread` is the discussion's
  /// number, `text` the message), through the planner's input; nothing is stored twice.
  | { type: "nudgeCard"; project: string; key: string; thread: number; text: string }
  /// The same for a message of the chat that is about no card: its text is typed again.
  | { type: "nudgeChat"; project: string; text: string }
  /// A message the assistant stopped before reading: post the same text again (starting the
  /// assistant) and stop counting the old one as unread.
  | { type: "sendAgain"; project: string; message: ConversationMessage };

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
  call: "load" | "start" | "stop" | "ask" | "discuss" | "askStory" | "nudge";
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

/// Start (`on`) or stop asking the daemon for the project's step every `STEP_POLL_MS`.
export type PlannerPoll = { type: "plannerPoll"; project: string; on: boolean };

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
      const { [msg.project]: _was, ...others } = s.planner.from;
      return [set({ error: "", sending: msg.project, from: msg.from ? { ...others, [msg.project]: msg.from } : others }), [call]];
    }
    case "plannerShown": {
      const n = (s.planner.shown[msg.project] ?? 0) + (msg.on ? 1 : -1);
      const { [msg.project]: _was, ...others } = s.planner.shown;
      return syncPoll(set({ shown: n > 0 ? { ...others, [msg.project]: n } : others }));
    }
    case "askStory": {
      const text = msg.text.trim();
      if (!text) return [s, []];
      const wait = !(plannerOf(s, msg.project)?.running ?? false);
      return [
        set({ error: "" }),
        [{ type: "plannerCall", project: msg.project, call: "askStory", card: msg.key, text, profile: s.planner.profile, wait }],
      ];
    }
    case "nudgeCard":
      return [
        set({ error: "" }),
        [{ type: "plannerCall", project: msg.project, call: "nudge", card: msg.key, thread: msg.thread, text: msg.text }],
      ];
    case "nudgeChat":
      return [set({ error: "" }), [{ type: "plannerCall", project: msg.project, call: "nudge", text: oneLine(msg.text) }]];
    case "sendAgain": {
      const { message: m } = msg;
      const about = m.about;
      // The draft place is its own, so what the person is typing now is not wiped by the send.
      const base = { type: "plannerCall", project: msg.project, profile: s.planner.profile, wait: !(plannerOf(s, msg.project)?.running ?? false), draft: `${msg.project}:again` } as const;
      const call: PlannerEffect =
        about?.kind === "card" ? { ...base, call: "discuss", card: about.key, text: m.text }
        : about ? { ...base, call: "ask", text: lineFor(about, m.text), raw: m.text, about }
        : { ...base, call: "ask", text: oneLine(m.text), raw: m.text };
      return [set({ error: "", resent: [...s.planner.resent, m.id] }), [call]];
    }
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
    case "wantPlannerTerminal":
      return [set({ terminalWanted: true }), []];
    case "plannerTerminalShown":
      return [set({ terminalWanted: false }), []];
    case "refreshPlanner":
      return [s, [loadPlanner(msg.project)]];
    case "plannerLoaded": {
      const seen = msg.state.running ? { ...s.planner.seen, [msg.project]: true } : s.planner.seen;
      // The assistant starting or stopping changes what runs in the project.
      const changed = (plannerOf(s, msg.project)?.running ?? false) !== msg.state.running;
      const fx: Effect[] = changed ? [{ type: "loadAgents" }] : [];
      const [next, poll] = syncPoll(set({ byProject: { ...s.planner.byProject, [msg.project]: msg.state }, seen }));
      return [next, [...fx, ...poll]];
    }
    case "plannerFailed":
      return [set({ error: msg.error, sending: undefined }), []];
  }
}

/// The assistant's turn started (`working`) or ended at `at`, as a `planner.turn` journal entry
/// says: the window follows it at once, and asks the daemon for the planner's state again (which
/// answers close their messages depends on the turn, see `openTurn`).
export function applyTurn(s: State, project: string, working: boolean, at: number): [State, Effect[]] {
  const was = plannerOf(s, project);
  if (!was) return [s, [loadPlanner(project)]];
  const now: PlannerState = working
    ? { ...was, running: true, working: true, since: was.working ? was.since : at }
    : { ...was, working: false, step: null, since: null };
  const [next, poll] = syncPoll({ ...s, planner: { ...s.planner, byProject: { ...s.planner.byProject, [project]: now } } });
  return [next, [loadPlanner(project), ...poll]];
}

/// Polls the step of each project whose turn is on while a chat or an ask box of it is on the
/// screen, and stops polling the rest. A turn still on that nothing shows any more drops its
/// step, since nobody follows it: the window never shows a step it has stopped asking for.
export function syncPoll(s: State): [State, Effect[]] {
  const { shown, polling, byProject } = s.planner;
  const want = Object.keys(shown).filter((p) => (shown[p] ?? 0) > 0 && byProject[p]?.working);
  const on = want.filter((p) => !polling.includes(p));
  const off = polling.filter((p) => !want.includes(p));
  if (on.length === 0 && off.length === 0) return [s, []];
  const dropped = Object.fromEntries(off.filter((p) => byProject[p]?.working).map((p) => [p, { ...byProject[p], step: null }]));
  const fx: Effect[] = [
    ...on.map((project): Effect => ({ type: "plannerPoll", project, on: true })),
    ...off.map((project): Effect => ({ type: "plannerPoll", project, on: false })),
  ];
  return [{ ...s, planner: { ...s.planner, polling: [...polling.filter((p) => !off.includes(p)), ...on], byProject: { ...byProject, ...dropped } } }, fx];
}

/// The planner of a project as last answered; not running until the daemon says so.
export const plannerOf = (s: State, project: string): PlannerState | undefined => s.planner.byProject[project];

/// What is typed to the assistant in a place, not sent yet.
export const draftOf = (s: State, where: string): string => s.planner.drafts[where] ?? "";

/// Whether the assistant's terminal belongs on screen: it runs, or this window saw it run.
export const showsTerminal = (s: State, project: string): boolean =>
  (plannerOf(s, project)?.running ?? false) || Boolean(s.planner.seen[project]);
