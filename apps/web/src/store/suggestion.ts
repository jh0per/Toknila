// The planner's standing suggestion as the window shows it. What the next step is, is the
// planner's call and core's check; here only the button of its action is decided.

import type { Suggestion } from "../api/types";
import type { Intent } from "./msg";
import type { Route } from "./route";
import type { State } from "./state";

/// The suggestions in the order the window lists them: one per project, in the sidebar's order.
export function suggestionsOf(s: State): Suggestion[] {
  return s.projects.flatMap((p) => s.suggestions.filter((x) => x.project === p.key));
}

/// The suggestion of one project, if it has one.
export function suggestionOfProject(s: State, project: string): Suggestion | undefined {
  return s.suggestions.find((x) => x.project === project);
}

/// What the person's click on the button does: the intent of the action. Start and Accept are the
/// card's own actions; Answer opens the card where the question is; Release opens the release
/// dialog of the chain.
export function suggestionIntents(s: State, x: Suggestion): Intent[] {
  const card = x.card;
  switch (x.action) {
    case "start": return card ? [{ type: "ticketAction", key: card, action: "start" }] : [];
    case "accept": return card ? [{ type: "ticketAction", key: card, action: "accept" }] : [];
    case "answer": {
      const route = suggestionRoute(s, x);
      return route ? [{ type: "navigate", route }] : [];
    }
    case "release": return x.chain ? [{ type: "openRelease", chain: x.chain }] : [];
    default: return [];
  }
}

/// The label of the button; none for a suggestion without an action.
export function suggestionLabel(x: Suggestion): string | undefined {
  switch (x.action) {
    case "start": return `Start ${x.card}`;
    case "accept": return `Accept ${x.card}`;
    case "answer": return `Answer ${x.card}`;
    case "release": return "Release";
    default: return undefined;
  }
}

/// The page of the suggestion's card, when the page knows its chain.
export function suggestionRoute(s: State, x: Suggestion): Route | undefined {
  const chain = x.chain ? s.chains[x.chain] : undefined;
  return chain && x.card ? { name: "card", project: x.project, chain: chain.chain.name, key: x.card } : undefined;
}
