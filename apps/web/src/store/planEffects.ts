// The network side of planning. Called by the effect runner; it decides nothing: every refusal
// comes from the daemon and goes to the reducer with the form it is about.

import { Refusal, patch, post } from "../api/http";
import type { ChainDetail, Project, TicketDetail } from "../api/types";
import type { Msg } from "./msg";
import type { PlanEffect, PlanForm } from "./plan";

type Dispatch = (msg: Msg) => void;

const enc = encodeURIComponent;

/// Runs one effect of planning; a refusal becomes `planRejected` for its form.
export function runPlanEffect(e: PlanEffect, dispatch: Dispatch): void {
  const reject = (form: PlanForm) => (err: unknown) =>
    dispatch({
      type: "planRejected",
      form,
      field: err instanceof Refusal ? err.field : undefined,
      message: err instanceof Error ? err.message : String(err),
    });
  switch (e.type) {
    case "createChain":
      post<ChainDetail>(`/projects/${enc(e.project)}/chains`, e.input)
        .then((chain) => dispatch({ type: "chainCreated", project: e.project, chain }))
        .catch(reject("chain"));
      return;
    case "createCard":
      post<TicketDetail>(`/chains/${enc(e.chain)}/tickets`, e.input)
        .then((detail) => dispatch({ type: "cardCreated", detail }))
        .catch(reject("card"));
      return;
    case "saveCard":
      patch<TicketDetail>(`/tickets/${enc(e.key)}`, e.patch)
        .then((detail) => dispatch({ type: "cardSaved", detail }))
        .catch(reject("edit"));
      return;
    case "moveCard":
      post<ChainDetail>(`/chains/${enc(e.chain)}/tickets/${enc(e.key)}/move`, { anchor: e.anchor, place: e.place })
        .then((chain) => dispatch({ type: "cardMoved", key: e.key, chain }))
        .catch(reject("move"));
      return;
    case "archiveProject":
      post<Project>(`/projects/${enc(e.project)}/${e.archived ? "archive" : "unarchive"}`, {})
        .then((project) => dispatch({ type: "projectArchived", project }))
        .catch(reject("archive"));
      return;
    case "hideProject":
      post<Project>(`/projects/${enc(e.project)}/${e.hidden ? "hide" : "unhide"}`, {})
        .then((project) => dispatch({ type: "projectHidden", project }))
        .catch(reject("hide"));
      return;
    case "clearHash":
      window.history.replaceState(window.history.state, "", `${window.location.pathname}${window.location.search}`);
      return;
  }
}
