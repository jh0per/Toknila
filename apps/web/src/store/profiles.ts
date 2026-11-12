// Which agent profiles a person sees by name. The simulated ones (`sim`, `fake...`) exist to try
// Toknila without a real agent, so they show in a development build, or where a project actually
// uses one; everywhere else a name is shown only when it is not the project's default agent.

/// Whether a profile only simulates an agent.
export const isSimulated = (name: string): boolean => name === "sim" || name.startsWith("sim-") || name.startsWith("fake");

/// The simulated agent: no process, no run log, no message; it exists for tests and `tok sim`.
export const SIM = "sim";

/// How the Default agent select names the simulated agent.
export const SIM_LABEL = "Simulated (no real agent, for tests)";

/// The agent a new project runs, sent explicitly by the add-a-project form and the onboarding.
export const REAL_AGENT = "claude-code";

/// What a select shows for a profile name.
export const profileLabel = (name: string): string => (name === SIM ? SIM_LABEL : name);

/// The names with `sim` last: it is chosen on purpose, never first.
export function simLast(names: string[]): string[] {
  return [...names.filter((n) => n !== SIM), ...names.filter((n) => n === SIM)];
}

/// Whether a card runs under the simulated agent: its own profile, else the project's default.
export const runsSimulated = (cardProfile: string | null | undefined, projectDefault: string | undefined): boolean =>
  (cardProfile ?? projectDefault) === SIM;

/// The banner of a project whose default agent is `sim`, or nothing.
export function simulatedBanner(project: { default_profile: string } | undefined): string | undefined {
  return project?.default_profile === SIM
    ? "This project runs a simulated agent: cards run without a real agent. Change it in Settings, Agents."
    : undefined;
}

/// The status word of a card: a running card under the simulated agent has nothing working on it,
/// so it never reads plain `running`.
export const statusWord = (status: string, simulated: boolean): string =>
  simulated && status === "running" ? "simulated" : status;

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
