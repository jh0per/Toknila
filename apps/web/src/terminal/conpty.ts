// The agents of a daemon on Windows run in ConPTY, which is not a byte pipe: it keeps only its
// screen (no lines above it) and repaints the whole screen, row by row from the top, after every
// resize. When the panel grows, xterm.js by default pulls lines down from its scrollback into the
// new rows, while ConPTY adds blank rows at the bottom; ConPTY's repaint then writes over the lines
// xterm pulled down, and they are gone. On GitHub's Windows runner that lost a line in every run of
// the copy test (TOKNILA-162: CI run 44308344208, the daemon's log held 80 of 80 lines, the page
// 79). With `windowsPty` set, xterm.js adds blank rows as ConPTY does.

import type { ITerminalOptions } from "@xterm/xterm";

/// Whether the browser runs on Windows ("Win32"). The daemon listens on this computer only, so its
/// agents run on the same system.
export const isWindows = (platform: string): boolean => /^win/i.test(platform);

/// The xterm.js options for the terminal of an agent on `platform`: ConPTY's on Windows, none
/// elsewhere (a Unix pty passes the program's bytes as they are; growing may show more history).
export function ptyOptions(platform: string): Pick<ITerminalOptions, "windowsPty"> {
  return isWindows(platform) ? { windowsPty: { backend: "conpty" } } : {};
}
