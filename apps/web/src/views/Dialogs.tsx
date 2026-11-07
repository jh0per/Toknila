import { useCallback } from "react";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { AddProject } from "./AddProject";
import { ImportChain } from "./ImportChain";
import { Modal } from "./Modal";
import { NewCard } from "./NewCard";
import { NewChain } from "./NewChain";

/// The dialog the store has open, if any. `inline` says the add-a-project form is already on the
/// page (onboarding), so it does not open a second time over it.
export function Dialogs({ state, dispatch, inline }: { state: State; dispatch: (i: Intent) => void; inline: boolean }) {
  const close = useCallback(() => dispatch({ type: "closeDialog" }), [dispatch]);
  const d = state.plan.dialog;
  if (!d) return null;
  switch (d.kind) {
    case "addProject":
      return inline ? null : (
        <Modal label="Add a project" testId="add-project-dialog" onClose={close}>
          <AddProject state={state} dispatch={dispatch} focus />
        </Modal>
      );
    case "newChain":
      return (
        <Modal label="New chain" testId="new-chain-dialog" onClose={close}>
          <NewChain project={d.project} state={state} dispatch={dispatch} />
        </Modal>
      );
    case "newCard": {
      const chain = state.chains[d.chain];
      return chain ? (
        <Modal label="New card" testId="new-card-dialog" onClose={close}>
          <NewCard chain={chain} state={state} dispatch={dispatch} />
        </Modal>
      ) : null;
    }
    case "import": {
      // Phase 1 offers the chain file; the import from a folder path and the list of found plans
      // join this dialog later.
      const project = state.projects.find((p) => p.key === d.project);
      return project ? (
        <Modal label="Import" testId="import-dialog" onClose={close}>
          <ImportChain project={project} state={state} dispatch={dispatch} />
        </Modal>
      ) : null;
    }
  }
}
