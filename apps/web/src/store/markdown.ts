// A small markdown reader for a chain's goal: paragraphs, bullet lists, headings, **bold** and
// `code`. Pure: it gives blocks, the view draws them as elements (nothing is ever injected as HTML).

export type Inline = { text: string; bold?: boolean; code?: boolean };
export type Block =
  | { kind: "p"; inline: Inline[] }
  | { kind: "h"; inline: Inline[] }
  | { kind: "ul"; items: Inline[][] };

/// `**bold**` and `` `code` `` inside a line.
export function inlines(line: string): Inline[] {
  const out: Inline[] = [];
  const re = /\*\*([^*]+)\*\*|`([^`]+)`/g;
  let at = 0;
  for (let m = re.exec(line); m; m = re.exec(line)) {
    if (m.index > at) out.push({ text: line.slice(at, m.index) });
    out.push(m[1] !== undefined ? { text: m[1], bold: true } : { text: m[2], code: true });
    at = m.index + m[0].length;
  }
  if (at < line.length) out.push({ text: line.slice(at) });
  return out;
}

/// The blocks of a text: blank lines separate paragraphs, `-` or `*` start a list, `#` a heading.
export function blocks(text: string): Block[] {
  const out: Block[] = [];
  let para: string[] = [];
  const flush = () => {
    if (para.length) out.push({ kind: "p", inline: inlines(para.join(" ")) });
    para = [];
  };
  for (const raw of text.replace(/\r\n/g, "\n").split("\n")) {
    const line = raw.trim();
    const item = /^[-*]\s+(.*)$/.exec(line);
    const head = /^#{1,6}\s+(.*)$/.exec(line);
    if (!line) flush();
    else if (item) {
      flush();
      const last = out[out.length - 1];
      if (last?.kind === "ul") last.items.push(inlines(item[1]));
      else out.push({ kind: "ul", items: [inlines(item[1])] });
    } else if (head) {
      flush();
      out.push({ kind: "h", inline: inlines(head[1]) });
    } else para.push(line);
  }
  flush();
  return out;
}
