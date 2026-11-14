import type { ConversationMessage } from "../api/types";
import { STILL_WORKING_MIN, minutesSince } from "../store/conversation";
import type { Intent } from "../store/msg";
import { NavLink } from "./Link";

/// The line under the chat while the assistant answers: how long it has taken, counting the
/// minutes, and the last thing the assistant said it was doing. After five minutes it says the
/// assistant is still working and links to the assistant's terminal, and stays so until the answer
/// comes. `since` is when the person's message was sent (`now` when it is still on its way).
export function WaitingLine({ project, since, now, progress, dispatch }: {
  project: string;
  since: number;
  now: number;
  progress?: ConversationMessage;
  dispatch: (i: Intent) => void;
}) {
  const minutes = minutesSince(since, now);
  return (
    <div className="chat-note" data-testid="chat-waiting" role="status">
      <p className="muted" data-testid="chat-waiting-text">
        {minutes >= STILL_WORKING_MIN ? (
          <>
            The assistant is still working ({minutes} min); its terminal is under{" "}
            <NavLink
              route={{ name: "planner", project }} dispatch={dispatch} testId="chat-terminal-link"
              also={{ type: "wantPlannerTerminal" }} title="Open the assistant's terminal"
            >
              Assistant → Terminal
            </NavLink>
          </>
        ) : minutes > 0 ? (
          `The assistant is answering… ${minutes} min`
        ) : (
          "The assistant is answering…"
        )}
      </p>
      {progress && <p className="muted chat-progress" data-testid="chat-progress">{progress.text}</p>}
    </div>
  );
}
