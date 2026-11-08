// Which agent profiles a person sees by name. The simulated ones (`sim`, `fake...`) exist to try
// Toknila without a real agent, so they show in a development build, or where a project actually
// uses one; everywhere else a name is shown only when it is not the project's default agent.

/// Whether a profile only simulates an agent.
export const isSimulated = (name: string): boolean => name === "sim" || name.startsWith("sim-") || name.startsWith("fake");

/// Whether a card's agent name is worth a chip: in a development build always, otherwise only when
/// it differs from the project's default agent.
export function showAgentName(name: string, projectDefault: string | undefined, dev: boolean): boolean {
  return dev || name !== projectDefault;
}

/// Whether a catalog entry is listed: real agents always, simulated ones in a development build or
/// when the project's default (or the one being edited) is that profile.
export function listProfile(name: string, current: string | undefined, dev: boolean): boolean {
  return !isSimulated(name) || dev || name === current;
}
