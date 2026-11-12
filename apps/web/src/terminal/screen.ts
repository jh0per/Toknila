// The terminal's text as plain text: the screen the person sees, or everything the window holds.
// Over the small part of xterm's buffer it needs, so it is tested without a browser.

export interface BufferLine {
  translateToString(trimRight?: boolean): string;
  isWrapped: boolean;
}

export interface TextBuffer {
  length: number;
  /// The first line of the visible screen.
  viewportY: number;
  getLine(y: number): BufferLine | undefined;
}

/// The lines `from` up to `to` (exclusive) as text: a wrapped line joins the one before it, the
/// trailing spaces of each row go, and so do the empty rows at the end.
function textOf(buffer: TextBuffer, from: number, to: number): string {
  const rows: string[] = [];
  for (let y = from; y < to; y++) {
    const line = buffer.getLine(y);
    if (!line) continue;
    const text = line.translateToString(true);
    if (line.isWrapped && rows.length > 0) rows[rows.length - 1] += text;
    else rows.push(text);
  }
  while (rows.length > 0 && rows[rows.length - 1] === "") rows.pop();
  return rows.join("\n");
}

/// What the screen shows now (scrolled where the person left it), `rows` lines tall.
export function visibleText(buffer: TextBuffer, rows: number): string {
  return textOf(buffer, buffer.viewportY, Math.min(buffer.length, buffer.viewportY + rows));
}

/// Everything the window holds: the scrollback and the screen.
export function allText(buffer: TextBuffer): string {
  return textOf(buffer, 0, buffer.length);
}
