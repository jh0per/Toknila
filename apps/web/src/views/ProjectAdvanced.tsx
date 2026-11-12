import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { ArchiveProject } from "./ArchiveProject";
import type { FormProps } from "./ProjectSections";
import { Strays } from "./Strays";
import { TrustNote } from "./TrustNote";
import { trustFacts } from "../store/trustAsked";

/// Where Toknila keeps its files on this machine.
function Places({ state }: { state: State }) {
  const places = state.settings.places;
  return (
    <details className="fold" data-testid="places">
      <summary>Where things live</summary>
      <dl className="facts">
        <dt>Data</dt>
        <dd data-testid="place-home"><code>{places?.home ?? "…"}</code></dd>
        <dt>Workspaces</dt>
        <dd data-testid="place-ships"><code>{places?.ships ?? "…"}</code></dd>
        <dt>Agent files</dt>
        <dd data-testid="place-profiles"><code>{places?.profiles ?? "…"}</code></dd>
      </dl>
      <p className="muted">The data folder holds the database and the run logs; a workspace is the copy of the project a card's agent works in.</p>
    </details>
  );
}

/// Advanced: the release branch when it is not the main branch, where things live, and the
/// danger zone (archive, delete), the last two folded away.
export function AdvancedSection({ project, state, dispatch }: FormProps & { dispatch: (i: Intent) => void }) {
  return (
    <div className="page-section" data-testid="project-advanced">
      {project.release_branch !== project.work_branch && (
        <dl className="facts">
          <dt>Release branch</dt>
          <dd data-testid="release-branch">{project.release_branch}</dd>
        </dl>
      )}
      <Places state={state} />
      <TrustNote {...trustFacts(state.events, state.settings.places?.ships, project.key)} />
      <Strays project={project.key} />
      <details className="fold danger-fold" data-testid="danger-zone">
        <summary>Danger zone</summary>
        <ArchiveProject project={project} state={state} dispatch={dispatch} />
      </details>
    </div>
  );
}
