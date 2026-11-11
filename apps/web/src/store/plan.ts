// Planning from the window: the dialogs (add a project, new chain, new card, import), editing and
// moving a card, archiving a project. Core decides what a card allows (`TicketDetail.can_edit`,
// `can_move`) and refuses the rest; this keeps what the daemon answered for the form it is about.

import type { ChainDetail, NewChain, NewTicket, PatchTicket, Place, Project, TicketDetail } from "../api/types";
import type { Effect } from "./msg";
import { upsert } from "./settings";
import type { State } from "./state";

/// A dialog over the window. `foundPlans` shows the plans of `imports.found`.
export type Dialog =
  | { kind: "addProject" }
  | { kind: "newChain"; project: string }
  | { kind: "newCard"; chain: string }
  | { kind: "import"; project: string }
  | { kind: "foundPlans" }
  | { kind: "deleteProject"; project: string }
  | { kind: "deleteChain"; chain: string }
  | { kind: "release"; chain: string };

/// The form a refusal belongs to.
export type PlanForm = "chain" | "card" | "edit" | "move" | "archive";

export interface PlanState {
  dialog: Dialog | undefined;
  /// The card (key) whose title and description are open for editing.
  editing: string | undefined;
  /// The card (key) whose new place is being picked.
  moving: string | undefined;
  busy: boolean;
  /// The last refusal, with the form and the field it is about.
  error: { form: PlanForm; field: string | undefined; message: string } | undefined;
  /// The sidebar lists the archived projects too.
  showArchived: boolean;
}

export const initialPlan: PlanState = {
  dialog: undefined, editing: undefined, moving: undefined, busy: false, error: undefined, showArchived: false,
};

/// The address hash the desktop menu sets to open the add-a-project form.
export const ADD_PROJECT_HASH = "#/add-project";

/// What the person did.
export type PlanIntent =
  | { type: "openDialog"; dialog: Dialog }
  | { type: "closeDialog" }
  | { type: "createChain"; project: string; input: NewChain }
  | { type: "createCard"; chain: string; input: NewTicket }
  | { type: "editCard"; key: string | undefined }
  | { type: "saveCard"; key: string; patch: PatchTicket }
  | { type: "pickPlace"; key: string | undefined }
  | { type: "moveCard"; chain: string; key: string; anchor: string; place: Place }
  | { type: "archiveProject"; project: string; archived: boolean }
  | { type: "showArchived"; on: boolean }
  | { type: "hashChanged"; hash: string };

/// What the daemon answered.
export type PlanResult =
  | { type: "chainCreated"; project: string; chain: ChainDetail }
  | { type: "cardCreated"; detail: TicketDetail }
  | { type: "cardSaved"; detail: TicketDetail }
  | { type: "cardMoved"; key: string; chain: ChainDetail }
  | { type: "projectArchived"; project: Project }
  | { type: "planRejected"; form: PlanForm; field: string | undefined; message: string };

export type PlanMsg = PlanIntent | PlanResult;

export type PlanEffect =
  | Extract<PlanIntent, { type: "createChain" | "createCard" | "saveCard" | "moveCard" | "archiveProject" }>
  | { type: "clearHash" };

/// The refusal of `form` (about `field`, or about no field when `field` is absent), as text.
export function planError(p: PlanState, form: PlanForm, field?: string): string | undefined {
  const e = p.error;
  return e && e.form === form && e.field === field ? e.message : undefined;
}

/// The chain with a fresh read model; a new chain joins the end of the list.
export function withChain(s: State, chain: ChainDetail): State {
  const id = chain.chain.id;
  const chainOrder = s.chainOrder.includes(id) ? s.chainOrder : [...s.chainOrder, id];
  return { ...s, chains: { ...s.chains, [id]: chain }, chainOrder };
}

/// The reducer of planning; `reduce` adds the moves of the page (to a new chain, a new card, home).
export function reducePlan(s: State, msg: PlanMsg): [State, Effect[]] {
  const set = (plan: Partial<PlanState>, base: State = s): State => ({ ...base, plan: { ...base.plan, ...plan } });
  const done = { busy: false, error: undefined };
  switch (msg.type) {
    case "openDialog": {
      const next = set({ dialog: msg.dialog, error: undefined });
      // A form opened again starts without the refusals of the last attempt.
      return [{ ...next, settings: { ...next.settings, problems: [], branches: [], error: undefined } }, []];
    }
    case "closeDialog":
      return [set({ dialog: undefined, error: undefined, busy: false }), []];
    case "hashChanged": {
      if (msg.hash !== ADD_PROJECT_HASH) return [s, []];
      // The hash is a request, not a place: it goes once the form is open, so the menu works again.
      const [next] = reducePlan(s, { type: "openDialog", dialog: { kind: "addProject" } });
      return [next, [{ type: "clearHash" }]];
    }
    case "editCard":
      return [set({ editing: msg.key, moving: undefined, error: undefined }), []];
    case "pickPlace":
      return [set({ moving: msg.key, editing: undefined, error: undefined }), []];
    case "showArchived":
      return [set({ showArchived: msg.on }), []];
    case "createChain":
    case "createCard":
    case "saveCard":
    case "moveCard":
    case "archiveProject":
      return [set({ busy: true, error: undefined }), [msg]];
    case "chainCreated":
      return [set({ ...done, dialog: undefined }, withChain(s, msg.chain)), []];
    case "cardCreated": {
      const d = msg.detail;
      const next = { ...s, details: { ...s.details, [d.ticket.key]: d } };
      return [set({ ...done, dialog: undefined }, next), [{ type: "loadChain", id: d.ticket.chain_id }]];
    }
    case "cardSaved": {
      const d = msg.detail;
      const next = { ...s, details: { ...s.details, [d.ticket.key]: d } };
      return [set({ ...done, editing: undefined }, next), [{ type: "loadChain", id: d.ticket.chain_id }]];
    }
    case "cardMoved":
      return [set({ ...done, moving: undefined }, withChain(s, msg.chain)), [{ type: "loadDetail", key: msg.key }]];
    case "projectArchived":
      return [set(done, { ...s, projects: upsert(s.projects, msg.project) }), [{ type: "loadAgents" }]];
    case "planRejected":
      return [set({ busy: false, error: { form: msg.form, field: msg.field, message: msg.message } }), []];
  }
}
