// API types, generated from the daemon's OpenAPI document (`npm run gen`). Never edit by hand:
// the document is pinned by a daemon test, so a change to the API shows up there first.

import type { components } from "../gen/api";

type Schemas = components["schemas"];

export type ChainDetail = Schemas["ChainDetail"];
export type ChainItem = Schemas["ChainItem"];
export type NodeRef = Schemas["NodeRef"];
export type Release = Schemas["Release"];
export type StatusReason = Schemas["StatusReason"];
export type Ticket = Schemas["Ticket"];
export type TicketItem = Schemas["TicketItem"];
export type ErrorBody = Schemas["ErrorBody"];

/// A journal entry; its payload is one domain event, read field by field where needed.
export type EventRecord = Omit<Schemas["EventRecord"], "payload"> & { payload: unknown };
export type Project = Schemas["Project"];
export type TicketDetail = Schemas["TicketDetail"];
export type Comment = Schemas["Comment"];
export type CommentKind = Schemas["CommentKind"];
export type Subtask = Schemas["Subtask"];
export type TicketStatus = Schemas["TicketStatus"];
export type TicketThreads = Schemas["TicketThreads"];
export type ThreadDetail = Schemas["ThreadDetail"];
export type ThreadVersion = Schemas["ThreadVersion"];
export type ThreadMessage = Schemas["ThreadMessage"];
export type OutOfOrder = Schemas["OutOfOrder"];
export type Ship = Schemas["Ship"];
export type Phase = Schemas["Phase"];
export type TicketAction = Schemas["TicketAction"];
export type ReviewView = Schemas["ReviewView"];
export type ReviewRound = Schemas["ReviewRound"];
export type ProjectAgents = Schemas["ProjectAgents"];
