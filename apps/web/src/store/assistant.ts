// The assistant panel beside the work: whether it is open, how wide, the "only about this" filter,
// and what it knows is open (the context chip). Open, width and filter are the viewer's own and
// are remembered in this browser (`assistantEffects.ts`); the chip is derived from the route, so
// it follows what is open, and taking it off lasts until something else opens.

import type { ConversationMessage } from "../api/types";
import { type ConversationMsg, reduceConversation } from "./conversation";
import type { Effect } from "./msg";
import type { Route } from "./route";
import type { State } from "./state";

/// What a message is about: the roadmap, a chain (by name) or a card (by key).
export type Context = { kind: "roadmap" } | { kind: "chain"; name: string } | { kind: "card"; key: string };

/// What an empty roadmap and an empty chat on the roadmap say to the person.
export const ROADMAP_STARTER = "Tell the assistant what to build; it splits it into releases";

export interface AssistantSlice {
  /// The panel beside the screens is open.
  open: boolean;
  /// Its width in pixels.
  width: number;
  /// The chat shows only the messages about the open thing.
  only: boolean;
  /// The person took the chip off; it comes back when something else opens.
  chipOff: boolean;
}

export const DEFAULT_WIDTH = 360;
export const MIN_WIDTH = 280;
export const MAX_WIDTH = 720;
/// Under this window width the panel overlays the screen instead of sitting beside it.
export const OVERLAY_PX = 1100;

export const initialAssistant: AssistantSlice = { open: false, width: DEFAULT_WIDTH, only: false, chipOff: false };

/// What the person did.
export type AssistantIntent =
  | { type: "toggleAssistant" }
  | { type: "openAssistant" }
  | { type: "closeAssistant" }
  | { type: "setAssistantWidth"; width: number }
  | { type: "setOnlyAboutThis"; only: boolean }
  | { type: "removeChip" };

export type AssistantEffect = { type: "saveAssistant"; open: boolean; width: number; only: boolean };

/// A width the panel can have.
export const clampWidth = (px: number): number => Math.round(Math.min(Math.max(px, MIN_WIDTH), MAX_WIDTH));

/// The slice a stored value (JSON) names; anything unreadable is the default.
export function parseAssistant(raw: string | null | undefined): AssistantSlice {
  try {
    const v = JSON.parse(raw ?? "") as Partial<AssistantSlice>;
    return {
      ...initialAssistant,
      open: v.open === true,
      only: v.only === true,
      width: typeof v.width === "number" && Number.isFinite(v.width) ? clampWidth(v.width) : DEFAULT_WIDTH,
    };
  } catch {
    return initialAssistant;
  }
}

/// What a route is about: the roadmap (a project's page), a chain, a card; nothing for the rest.
export function contextOf(route: Route): Context | undefined {
  switch (route.name) {
    case "project": return { kind: "roadmap" };
    case "chain":
    case "projectChain": return { kind: "chain", name: route.chain };
    case "card": return { kind: "card", key: route.key };
    default: return undefined;
  }
}

/// The context chip: what is open, unless the person took it off.
export const chipOf = (s: State): Context | undefined => (s.assistant.chipOff ? undefined : contextOf(s.route));

/// The chip of a message sent from the panel: only while the panel is open beside the work (a box
/// elsewhere, with the panel shut, is a plain message).
export const panelChip = (s: State): Context | undefined => (s.assistant.open ? chipOf(s) : undefined);

/// The same context as one comparable string.
export const contextKey = (c: Context): string => (c.kind === "roadmap" ? "roadmap" : c.kind === "chain" ? `chain:${c.name}` : `card:${c.key}`);

/// What the chip says.
export const contextLabel = (c: Context): string =>
  c.kind === "roadmap" ? "Roadmap" : c.kind === "chain" ? `the epic ${c.name}` : `the story ${c.key}`;

/// The slice after the route changed: whatever opens next brings its chip back.
export const chipBack = (a: AssistantSlice): AssistantSlice => (a.chipOff ? { ...a, chipOff: false } : a);

/// The line the planner's terminal takes for a message about a context, on one line. A card's
/// line is `Discussion on story KEY (thread N): …` (`askLine` in discussion.ts), as the card's
/// thread is the daemon's to number.
export function lineFor(c: Context, text: string): string {
  const one = text.trim().replace(/\s*\n\s*/g, " ");
  return c.kind === "chain" ? `About epic ${c.name}: ${one}` : `About the roadmap: ${one}`;
}

/// The messages "only about this" keeps: those about the card, the chain or the roadmap.
export function aboutThis(messages: ConversationMessage[], c: Context): ConversationMessage[] {
  return messages.filter((m) => {
    if (c.kind === "card") return m.about?.kind === "card" && m.about.key === c.key;
    if (c.kind === "chain") return m.about?.kind === "chain" && m.about.name === c.name;
    return m.about?.kind === "roadmap";
  });
}

/// Whether a route has a project screen the panel can sit beside.
export const beside = (route: Route): boolean => contextOf(route) !== undefined;

/// The state with the panel open, and the effect that remembers it.
export const withPanelOpen = (s: State): [State, Effect[]] => {
  const a = { ...s.assistant, open: true };
  return [{ ...s, assistant: a }, [{ type: "saveAssistant", open: true, width: a.width, only: a.only }]];
};

/// Where a message sent from a box goes: the chat is in the panel beside the work or on the full
/// screen; from anywhere else the panel opens (on the roadmap, if the screen cannot have it).
export function showChat(before: State, next: State, fx: Effect[], project: string): [State, Effect[], Route | undefined] {
  const there = (before.route.name === "planner" && before.route.project === project) || (before.assistant.open && beside(before.route));
  if (!fx.length || there) return [next, fx, undefined];
  const [shown, save] = withPanelOpen(next);
  return [shown, [...fx, ...save], beside(before.route) ? undefined : { name: "project", project }];
}

export function reduceAssistant(s: State, msg: AssistantIntent | ConversationMsg): [State, Effect[]] {
  const set = (patch: Partial<AssistantSlice>): [State, Effect[]] => {
    const a = { ...s.assistant, ...patch };
    return [{ ...s, assistant: a }, [{ type: "saveAssistant", open: a.open, width: a.width, only: a.only }]];
  };
  switch (msg.type) {
    case "toggleAssistant": return set({ open: !s.assistant.open });
    case "openAssistant": return s.assistant.open ? [s, []] : set({ open: true });
    case "closeAssistant": return s.assistant.open ? set({ open: false }) : [s, []];
    case "setAssistantWidth": return set({ width: clampWidth(msg.width) });
    case "setOnlyAboutThis": return set({ only: msg.only });
    case "removeChip": return [{ ...s, assistant: { ...s.assistant, chipOff: true } }, []];
    default: return reduceConversation(s, msg);
  }
}
