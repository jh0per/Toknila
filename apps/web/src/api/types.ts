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
export type InboxItem = Schemas["InboxItem"];
export type InboxKind = Schemas["InboxKind"];
export type AnswerKind = Schemas["AnswerKind"];
export type AgentRole = Schemas["AgentRole"];
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
export type SystemChecks = Schemas["SystemChecks"];
export type Check = Schemas["Check"];
export type CliLink = Schemas["CliLink"];
export type RepoCheck = Schemas["RepoCheck"];
export type RepoProblem = Schemas["RepoProblem"];
export type ListedProfile = Schemas["ListedProfile"];
export type ProfileList = Schemas["ProfileList"];
export type PatchProject = Schemas["PatchProject"];
export type ReviewMode = Schemas["ReviewMode"];
export type MergeMode = Schemas["MergeMode"];
export type MergeMethod = Schemas["MergeMethod"];
export type PullRequest = Schemas["PullRequest"];
export type UsageView = Schemas["UsageView"];
export type UsageSummary = Schemas["UsageSummary"];
export type UsageFigures = Schemas["UsageFigures"];
export type UsageItem = Schemas["UsageItem"];
export type DayUsage = Schemas["DayUsage"];
export type QuotaWindow = Schemas["QuotaWindow"];
export type NewChain = Schemas["NewChain"];
export type NewTicket = Schemas["NewTicket"];
export type PatchTicket = Schemas["PatchTicket"];
export type Place = Schemas["Place"];
export type FoundPlan = Schemas["FoundPlan"];
export type PlanScan = Schemas["PlanScan"];
export type ImportPathResult = Schemas["ImportPathResult"];
export type ImportedChain = Schemas["ImportedChain"];
export type FolderNote = Schemas["FolderNote"];
export type ProjectDeletion = Schemas["ProjectDeletion"];
