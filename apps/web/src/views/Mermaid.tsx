import { useEffect, useState } from "react";

let seq = 0;

/// A Mermaid source drawn as a diagram. The library loads on first use; `strict` keeps scripts and
/// click handlers out of what it produces.
export function Mermaid({ source }: { source: string }) {
  const [svg, setSvg] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    let live = true;
    setError("");
    import("mermaid")
      .then(async ({ default: mermaid }) => {
        mermaid.initialize({ startOnLoad: false, securityLevel: "strict" });
        const out = await mermaid.render(`mmd-${++seq}`, source);
        if (live) setSvg(out.svg);
      })
      .catch((e) => live && setError(String(e instanceof Error ? e.message : e)));
    return () => {
      live = false;
    };
  }, [source]);
  if (error) return <pre className="art-text error" data-testid="mermaid-error">{error}</pre>;
  if (!svg) return <p className="muted">Drawing…</p>;
  return <div className="art-mermaid" data-testid="mermaid" dangerouslySetInnerHTML={{ __html: svg }} />;
}
