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
