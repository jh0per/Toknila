import { useCallback, useEffect, useState } from "react";
import { get, post } from "../api/http";
import type { Workspaces as WorkspaceList, WorkspacesRetired } from "../api/types";
import { retiredText, workspaceText, workspacesLine } from "../store/workspaces";

/// The workspaces of a project's stories: how many there are, how many belong to finished
/// stories, and Clean up, which retires those (worktree, branch when it holds nothing unmerged,
/// record). The assistant's reading copy is listed and named, never counted or cleaned.
export function Workspaces({ project }: { project: string }) {
  const [list, setList] = useState<WorkspaceList | undefined>();
  const [note, setNote] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);
  const load = useCallback(async () => {
    try {
      setList(await get<WorkspaceList>(`/projects/${project}/ships`));
      setError(undefined);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [project]);
  useEffect(() => {
    void load();
  }, [load]);
  const clean = async () => {
    setBusy(true);
    setNote(undefined);
    try {
      const done = await post<WorkspacesRetired>(`/projects/${project}/ships/retire-finished`, {});
      setNote(retiredText(done.retired.length));
      // The retirement runs right after the request; look again shortly.
      await load();
      setTimeout(() => void load(), 1500);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <section data-testid="workspaces">
      <h3 className="sub">Workspaces</h3>
      {list && <p data-testid="workspaces-line">{workspacesLine(list)}</p>}
      {list && list.items.length > 0 && (
        <ul data-testid="workspaces-list">
          {list.items.map((w) => <li key={w.path}><code>{workspaceText(w)}</code></li>)}
        </ul>
      )}
      {list && list.finished > 0 && (
        <button type="button" className="btn" disabled={busy} data-testid="workspaces-clean" onClick={() => void clean()}>
          {busy ? "Cleaning…" : "Clean up"}
        </button>
      )}
      {note && <p className="muted" role="status" data-testid="workspaces-note">{note}</p>}
      {error && <p className="field-error" role="alert" data-testid="workspaces-error">{error}</p>}
    </section>
  );
}
