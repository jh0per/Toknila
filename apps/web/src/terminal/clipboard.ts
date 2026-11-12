/// Puts `text` on the clipboard; `false` when the browser or the webview refuses (no user gesture,
/// no permission, no clipboard API). Never throws.
export async function writeClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
