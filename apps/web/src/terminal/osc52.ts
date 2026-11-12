// OSC 52: a program asks the terminal to put text on the clipboard (`ESC ] 52 ; c ; <base64> BEL`).
// xterm hands the handler what follows `52;`: the selection letters, `;`, the payload.

/// The text of an OSC 52 write, or `null` for anything else: a query (`?`), an empty payload (a
/// clear), invalid base64 or invalid UTF-8. Reading the clipboard for the agent is never offered.
export function decodeOsc52(data: string): string | null {
  const at = data.indexOf(";");
  if (at < 0) return null;
  const payload = data.slice(at + 1).trim();
  if (payload === "" || payload === "?") return null;
  try {
    const bin = atob(payload);
    const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    return null;
  }
}
