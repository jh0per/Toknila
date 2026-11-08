// Updates of the desktop app. The shell decides everything (when to check, what the banner says,
// whether a restart must warn); this slice only keeps what it answered and sends the person's
// choices back. In a browser there is no shell, nothing is shown and nothing is asked.

/// What the shell reports (`update_status`, `check_for_updates`).
export interface UpdateView {
  current_version: string;
  /// Unix seconds of the last finished check.
  last_check: number | null;
  /// A check or a download is under way.
  busy: boolean;
  /// Set when an update is downloaded and waits for a restart.
  banner: { title: string; notes: string } | null;
  error: string | null;
}

/// What the shell answers to "restart to update".
export type RestartAnswer = { outcome: "go" } | { outcome: "warn"; message: string };

export interface UpdatesState {
  view: UpdateView | undefined;
  /// The sentence shown before a restart that would stop agents.
  warning: string | undefined;
  /// The banner title the person dismissed with "Later"; a newer version shows again.
  dismissed: string | undefined;
  /// Why the last restart or check failed to start.
  error: string | undefined;
}

export const initialUpdates: UpdatesState = {
  view: undefined,
  warning: undefined,
  dismissed: undefined,
  error: undefined,
};

export type UpdateIntent =
  | { type: "checkForUpdates" }
  | { type: "restartToUpdate" }
  | { type: "restartAnyway" }
  | { type: "updateLater" };

export type UpdateResult =
  | { type: "updateViewLoaded"; view: UpdateView }
  | { type: "updateWarned"; message: string }
  | { type: "updateFailed"; message: string };

export type UpdateMsg = UpdateIntent | UpdateResult;

export type UpdateEffect =
  | { type: "watchUpdates" }
  | { type: "checkForUpdates" }
  | { type: "restartToUpdate"; confirmed: boolean };

/// The banner to show now, if any.
export function banner(u: UpdatesState): { title: string; notes: string } | undefined {
  const b = u.view?.banner;
  return b && b.title !== u.dismissed ? b : undefined;
}

/// Whether an update waits for a restart, dismissed banner or not: the dot on Settings stays until
/// the app has restarted into it.
export const updateReady = (u: UpdatesState): boolean => Boolean(u.view?.banner);

type S = { updates: UpdatesState };

/// The reducer of this slice.
export function reduceUpdates<T extends S>(s: T, msg: UpdateMsg): [T, UpdateEffect[]] {
  const set = (p: Partial<UpdatesState>): T => ({ ...s, updates: { ...s.updates, ...p } });
  switch (msg.type) {
    case "checkForUpdates":
      return [set({ error: undefined }), [{ type: "checkForUpdates" }]];
    case "restartToUpdate":
      return [set({ error: undefined }), [{ type: "restartToUpdate", confirmed: false }]];
    case "restartAnyway":
      return [set({ warning: undefined, error: undefined }), [{ type: "restartToUpdate", confirmed: true }]];
    case "updateLater":
      return [set({ warning: undefined, dismissed: s.updates.view?.banner?.title }), []];
    case "updateViewLoaded":
      return [set({ view: msg.view }), []];
    case "updateWarned":
      return [set({ warning: msg.message }), []];
    case "updateFailed":
      return [set({ error: msg.message }), []];
  }
}

/// "Last checked …" for About, from the shell's Unix seconds.
export function lastCheckText(unix: number | null | undefined): string {
  if (unix == null) return "Not checked yet";
  return `Last checked ${new Date(unix * 1000).toLocaleString()}`;
}
