// The colour theme: follow the system, or force light or dark. It is the viewer's own choice, kept
// in this browser (`themeEffects.ts`), never sent to the daemon.

import type { Effect } from "./msg";
import type { State } from "./state";

export type Theme = "system" | "light" | "dark";

export const THEMES: Theme[] = ["system", "light", "dark"];

export const THEME_LABEL: Record<Theme, string> = { system: "System", light: "Light", dark: "Dark" };

/// The theme a stored value names; anything else (nothing stored, a stale value) follows the system.
export function parseTheme(raw: string | null | undefined): Theme {
  return THEMES.find((t) => t === raw) ?? "system";
}

/// What the person did.
export type ThemeIntent = { type: "setTheme"; theme: Theme };

export type ThemeEffect = { type: "saveTheme"; theme: Theme };

/// The reducer of the theme switch: the choice is state, remembering it is an effect.
export function reduceTheme(s: State, msg: ThemeIntent): [State, Effect[]] {
  return [{ ...s, theme: msg.theme }, [{ type: "saveTheme", theme: msg.theme }]];
}
