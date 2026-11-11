import { type Block, type Inline, blocks } from "../store/markdown";

const span = (parts: Inline[]) =>
  parts.map((p, i) => (p.bold ? <b key={i}>{p.text}</b> : p.code ? <code key={i}>{p.text}</code> : <span key={i}>{p.text}</span>));

/// A goal's markdown as elements.
export function Markdown({ text }: { text: string }) {
  return (
    <div className="md">
      {blocks(text).map((b: Block, i) =>
        b.kind === "ul" ? <ul key={i}>{b.items.map((it, j) => <li key={j}>{span(it)}</li>)}</ul>
        : b.kind === "h" ? <p key={i} className="md-h"><b>{span(b.inline)}</b></p>
        : <p key={i}>{span(b.inline)}</p>)}
    </div>
  );
}
