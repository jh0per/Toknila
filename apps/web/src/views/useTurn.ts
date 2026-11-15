import { useEffect, useState } from "react";
import { talkOf, unanswered, wakeWhenLate } from "../store/conversation";
import type { Intent } from "../store/msg";
import { plannerOf } from "../store/planner";
import { type TurnView, lastTurnEnded, turnView } from "../store/plannerTurn";
import type { State } from "../store/state";

/// How often the waiting line looks at the clock to count the minutes.
const TICK_MS = 10_000;

/// The project's assistant's turn as the chat and the ask boxes say it, with the clock it is read
/// at: the clock ticks while the waiting line counts the minutes, and wakes when the unanswered
/// message becomes overdue.
export function useTurn(state: State, project: string): TurnView & { now: number } {
  const messages = talkOf(state, project)?.messages ?? [];
  const [now, setNow] = useState(() => Date.now());
  const owed = unanswered(messages);
  useEffect(() => {
    setNow(Date.now());
    return wakeWhenLate(messages, Date.now(), () => setNow(Date.now()));
  }, [owed?.id]);
  const view = turnView(messages, plannerOf(state, project), lastTurnEnded(state, project), state.planner.sending === project, now, state.planner.resent);
  useEffect(() => {
    if (!view.waiting) return;
    setNow(Date.now());
    const id = setInterval(() => setNow(Date.now()), TICK_MS);
    return () => clearInterval(id);
  }, [view.waiting, view.since]);
  return { ...view, now };
}

/// Says the window shows the project's assistant (a chat, an ask box) while `on`, so the step of
/// its turn is asked for, and asks for its state when it comes on the screen.
export function usePlannerShown(project: string, dispatch: (i: Intent) => void, on = true): void {
  useEffect(() => {
    if (!on) return;
    dispatch({ type: "plannerShown", project, on: true });
    dispatch({ type: "refreshPlanner", project });
    return () => dispatch({ type: "plannerShown", project, on: false });
  }, [project, on]);
}
