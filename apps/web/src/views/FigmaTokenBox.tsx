import { useState } from "react";
import { sendEmpty } from "../api/designs";

/// The project's Figma personal access token. It is sent to the daemon, which keeps it in a file
/// of its own; the page never gets it back, so the field is empty and only says whether one is saved.
export function FigmaTokenBox({ project, saved, onChange }: { project: string; saved: boolean; onChange: () => Promise<void> }) {
  const [text, setText] = useState("");
  const [error, setError] = useState<string | undefined>();
  const path = `/projects/${project}/figma-token`;
  const act = async (what: () => Promise<void>) => {
    setError(undefined);
    try {
      await what();
      setText("");
      await onChange();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  return (
    <section className="card-block" data-testid="figma-token">
      <h2>Figma token</h2>
      <p className="muted">
        With a personal access token Toknila fetches a linked frame as a png. Without one the link is kept and the
        frame has to be exported by hand. The token is stored on this computer, outside the repository.
      </p>
      <p data-testid="figma-token-state">{saved ? "A token is saved." : "No token saved."}</p>
      <div className="design-add">
        <input
          type="password" autoComplete="off" spellCheck={false} value={text} placeholder="figd_…"
          data-testid="figma-token-input" aria-label="Figma personal access token" onChange={(e) => setText(e.target.value)}
        />
        <button
          type="button" className="btn primary" disabled={!text.trim()} data-testid="figma-token-save"
          onClick={() => void act(() => sendEmpty("PUT", path, { token: text }))}
        >
          Save token
        </button>
        {saved && (
          <button type="button" className="btn" data-testid="figma-token-clear" onClick={() => void act(() => sendEmpty("DELETE", path))}>
            Remove token
          </button>
        )}
      </div>
      {error && <p className="field-error" role="alert" data-testid="figma-token-error">{error}</p>}
    </section>
  );
}
