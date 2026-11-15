import { type ReactNode, useEffect } from "react";
import { openTurn, talkOf } from "../store/conversation";
import type { Intent } from "../store/msg";
import { draftOf } from "../store/planner";
import { nudgeFor } from "../store/plannerTurn";
import type { State } from "../store/state";
import { AskBox, type AskBoxProps } from "./AskBox";
import { usePlannerShown, useTurn } from "./useTurn";
import { EndedLine, WaitingLine } from "./WaitingLine";

/// An ask box outside the chat (an empty epic's, an empty roadmap's). After a send it shows what
/// was sent from it and the chat's waiting line instead of the text box, until the assistant
/// answers, the first story or epic appears (the empty screen and the box go), or the turn ends;
/// a turn that ended without an answer is said above the box, with Nudge. A send that failed
/// leaves the box as it was, its text kept, and the panel says why.
export function AskBoxOutside(props: AskBoxProps & { place: string }) {
  const { project, state, dispatch, testId, place } = props;
  usePlannerShown(project, dispatch);
  useEffect(() => {
    if (!talkOf(state, project)) dispatch({ type: "loadConversation", project });
  }, [project]);
  const turn = useTurn(state, project);
  const sending = state.planner.sending === project;
  // What was sent from here: the text on its way, then the stored message the assistant owes.
  const sent = state.planner.from[project] !== place ? undefined : sending ? draftOf(state, project) : turn.owed?.text;
  const shown = sent ? <AskSent project={project} text={sent} turn={turn} state={state} dispatch={dispatch} testId={testId} /> : null;
  if (shown && (sending || turn.waiting)) return shown;
  return (
    <>
      {turn.ended && shown}
      <AskBox {...props} />
    </>
  );
}

/// What was sent from an ask box outside the chat: the text and, under it, the chat's waiting line
/// (or, once the turn ended without an answer, the chat's stopped line with Nudge), with a link
/// that opens the chat beside the work.
export function AskSent({ project, text, turn, state, dispatch, testId }: {
  project: string;
  text: string;
  turn: ReturnType<typeof useTurn>;
  state: State;
  dispatch: (i: Intent) => void;
  testId: string;
}) {
  const open: ReactNode = (
    <button type="button" className="inline-link" data-testid={`${testId}-open-chat`} onClick={() => dispatch({ type: "openAssistant" })}>
      Open the chat
    </button>
  );
  const progress = openTurn(talkOf(state, project)?.messages ?? []).progress;
  const owed = turn.owed;
  return (
    <div className="ask-sent" data-testid={`${testId}-sent`}>
      <p className="ask-sent-label">Sent to the assistant</p>
      <p className="ask-sent-text" data-testid={`${testId}-sent-text`}>{text}</p>
      {turn.ended && owed ? (
        <EndedLine testId={testId} onNudge={() => dispatch(nudgeFor(project, owed))}>{open}</EndedLine>
      ) : (
        <>
          <WaitingLine
            testId={testId} project={project} since={turn.since} now={turn.now} step={turn.step} dispatch={dispatch}
            progress={turn.waiting ? progress : undefined}
          />
          <p className="ask-sent-open">{open}</p>
        </>
      )}
    </div>
  );
}
