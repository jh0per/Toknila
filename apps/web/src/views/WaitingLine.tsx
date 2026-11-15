import type { ReactNode } from "react";
import type { ConversationMessage } from "../api/types";
import { STILL_WORKING_MIN, minutesSince } from "../store/conversation";
import type { Intent } from "../store/msg";
import { NavLink } from "./Link";

/// The line under the chat while the assistant answers: how long it has taken, counting the
/// minutes, what the assistant does now as the daemon says (`step`, beside the minutes), and the
/// last thing the assistant said it was doing (under it). After five minutes it says the assistant
/// is still working and links to the assistant's terminal, and stays so until the answer comes.
/// `since` is when the person's message was sent (`now` when it is still on its way). Only the
/// count is a live region: a new step is not announced. `testId` names an ask box's copy.
export function WaitingLine({ project, since, now, progress, step, dispatch, testId = "chat" }: {
  project: string;
  since: number;
  now: number;
  progress?: ConversationMessage;
  step?: string | null;
  dispatch: (i: Intent) => void;
  testId?: string;
}) {
  const minutes = minutesSince(since, now);
  return (
    <div className="chat-note" data-testid={`${testId}-waiting`}>
      <p className="muted chat-waiting">
        <span role="status" data-testid={`${testId}-waiting-text`}>
          {minutes >= STILL_WORKING_MIN ? (
            <>
              The assistant is still working ({minutes} min); its terminal is under{" "}
              <NavLink
                route={{ name: "planner", project }} dispatch={dispatch} testId={`${testId}-terminal-link`}
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
        </span>
        {step && <> <span className="chat-step" data-testid={`${testId}-step`} title={step}>{`· ${step}`}</span></>}
      </p>
      {progress && <p className="muted chat-progress" data-testid={`${testId}-progress`}>{progress.text}</p>}
    </div>
  );
}

/// The line of a turn that ended while the person's last message had no answer: said at once,
/// with Nudge. `children` sit beside the button (an ask box's link to the chat).
export function StoppedLine({ onNudge, testId = "chat", children }: { onNudge: () => void; testId?: string; children?: ReactNode }) {
  return (
    <div className="chat-note chat-stopped" data-testid={`${testId}-stopped`} role="status">
      <p className="muted"><b>The assistant stopped without answering.</b></p>
      <div className="row chat-stopped-row">
        <p className="muted">Nudge sends your message to the assistant again.</p>
        <button type="button" className="btn" data-testid={`${testId}-nudge`} onClick={onNudge}>Nudge</button>
        {children}
      </div>
    </div>
  );
}
