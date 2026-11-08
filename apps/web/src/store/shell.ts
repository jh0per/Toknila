// The page's door to the desktop shell: Tauri IPC when the page runs inside the Toknila app,
// nothing in a browser. The shell allows only the commands named in its capability file for the
// daemon's loopback page (updates and the folder picker); everything else is out of reach.

export type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

declare global {
  interface Window {
    __TAURI__?: { core?: { invoke?: Invoke } };
  }
}

/// The shell's `invoke`, when the page runs inside the desktop app.
export const shellInvoke = (): Invoke | undefined => window.__TAURI__?.core?.invoke;

/// Whether the page can ask the shell for a folder (the Browse button is shown only then).
export const canBrowse = (): boolean => shellInvoke() !== undefined;

/// Asks the shell to show its folder picker; the chosen path, or undefined when the person
/// cancelled or there is no shell.
export async function pickFolder(): Promise<string | undefined> {
  const call = shellInvoke();
  if (!call) return undefined;
  const chosen = await call("pick_folder");
  return typeof chosen === "string" && chosen !== "" ? chosen : undefined;
}
