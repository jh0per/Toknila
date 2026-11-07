import { useCallback } from "react";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { AddProject } from "./AddProject";
import { DeleteProject } from "./DeleteProject";
import { FoundPlans } from "./FoundPlans";
import { ImportDialog } from "./ImportDialog";
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
    case "import":
      return (
        <Modal label="Import" testId="import-dialog" onClose={close}>
          <ImportDialog project={d.project} state={state} dispatch={dispatch} />
        </Modal>
      );
    case "foundPlans": {
      const project = state.imports.found?.project;
      return project ? (
        <Modal label="Plans in this repository" testId="found-plans-dialog" onClose={close}>
          <div className="card-block form">
            <h2>Plans in {project}'s repository</h2>
            <FoundPlans project={project} state={state} dispatch={dispatch} />
          </div>
        </Modal>
      ) : null;
    }
    case "deleteProject":
      return (
        <Modal label="Delete a project" testId="delete-dialog" onClose={close}>
          <DeleteProject project={d.project} state={state} dispatch={dispatch} />
        </Modal>
      );
  }
}
