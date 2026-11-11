// Everything that changes the state: what the person did (intents), API answers, the stream.

import type {
  AgentRole, AnswerKind, ChainDetail, CommentKind, EventRecord, InboxItem, Project, ProjectAgents, Suggestion, TicketAction, TicketDetail,
  TicketThreads,
} from "../api/types";
import type { AutostartEffect, AutostartIntent, AutostartResult } from "./autostart";
import type { EdgeEffect, EdgeIntent, EdgeResult } from "./edges";
import type { EpicEffect, EpicIntent, EpicResult } from "./epic";
import type { PlacementEffect, PlacementIntent, PlacementResult } from "./placement";
import type { ImportEffect, ImportIntent, ImportResult } from "./imports";
import type { PlanEffect, PlanIntent, PlanResult } from "./plan";
import type { QuickIntent } from "./quick";
import type { PlannerEffect, PlannerIntent, PlannerResult } from "./planner";
import type { ReleaseEffect, ReleaseIntent, ReleaseResult } from "./release";
import type { RemovalEffect, RemovalIntent, RemovalResult } from "./removal";
import type { Route, Tab } from "./route";
import type { SettingsEffect, SettingsIntent, SettingsResult } from "./settings";
import type { ShortcutIntent } from "./shortcuts";
import type { ThemeEffect, ThemeIntent } from "./theme";
import type { UpdateEffect, UpdateIntent, UpdateResult } from "./updates";
import type { UsageEffect, UsageIntent, UsageResult } from "./usage";
import type { FollowEffect, FollowIntent, FollowResult } from "./follow";

/// What the person did.
export type Intent =
  | { type: "navigate"; route: Route }
  | { type: "selectCard"; project: string; chain: string; key: string }
  | { type: "selectTab"; tab: Tab }
  | { type: "pickVersion"; thread: string; number: number }
  | CardCommand
  | ThreadAction
  | SettingsIntent
  | PlanIntent
  | PlannerIntent
  | ImportIntent
  | RemovalIntent
  | ReleaseIntent
  | EpicIntent
  | EdgeIntent
  | PlacementIntent
  | AutostartIntent
  | QuickIntent
  | ThemeIntent
  | ShortcutIntent
  | UpdateIntent
  | UsageIntent
  | FollowIntent;

/// What a person decides about the open subtasks left behind a card done out of order.
export type LeftoverAction = "move" | "close" | "leave";

/// What a person does to a card: an action of its status (a return carries the comment), or a
/// comment of the template's kinds.
export type CardCommand =
  | { type: "ticketAction"; key: string; action: TicketAction; text?: string }
  | { type: "leftovers"; key: string; action: LeftoverAction }
  | { type: "addComment"; key: string; kind: CommentKind; text: string }
  | { type: "answer"; key: string; role: AgentRole; answer: AnswerKind; text?: string; always?: boolean };

/// What a person does in a thread; `text` is the message or the comment of a request for changes.
export type ThreadAction = {
  type: "threadAction";
  key: string;
  thread: string;
  action: "message" | "changes" | "approve" | "accept_edit" | "reject_edit";
  text?: string;
};

export type Msg =
  | Intent
  | { type: "started" }
  | { type: "journalTail"; seq: number; events: EventRecord[] }
  | { type: "loaded"; projects: Project[]; chains: ChainDetail[] }
  | { type: "projectsLoaded"; projects: Project[] }
  | { type: "agentsLoaded"; agents: ProjectAgents[] }
  | { type: "chainLoaded"; chain: ChainDetail }
  | { type: "inboxLoaded"; items: InboxItem[] }
  | { type: "suggestionsLoaded"; suggestions: Suggestion[] }
  | { type: "detailLoaded"; detail: TicketDetail }
  | { type: "threadsLoaded"; threads: TicketThreads }
  | { type: "loadFailed"; error: string }
  | { type: "linkOpened" }
  | { type: "linkClosed" }
  | { type: "event"; event: EventRecord }
  | { type: "urlChanged"; route: Route }
  | SettingsResult
  | PlanResult
  | PlannerResult
  | ImportResult
  | RemovalResult
  | ReleaseResult
  | EpicResult
  | EdgeResult
  | PlacementResult
  | AutostartResult
  | UpdateResult
  | UsageResult
  | FollowResult;

/// A side effect for the runner; the reducer only describes it.
export type Effect =
  | { type: "fetchTail" }
  | { type: "loadAll" }
  | { type: "loadProjects" }
  | { type: "loadAgents" }
  | { type: "loadChain"; id: string }
  | { type: "loadInbox" }
  | { type: "loadDetail"; key: string }
  | { type: "loadThreads"; key: string }
  | CardCommand
  | ThreadAction
  | SettingsEffect
  | PlanEffect
  | PlannerEffect
  | ImportEffect
  | RemovalEffect
  | ReleaseEffect
  | EpicEffect
  | EdgeEffect
  | PlacementEffect
  | AutostartEffect
  | ThemeEffect
  | UpdateEffect
  | UsageEffect
  | FollowEffect
  | { type: "connect"; since: number; delayMs: number }
  | { type: "pushUrl"; path: string };
