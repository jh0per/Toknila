import { ESTIMATE_NOTE, costText } from "../format";
import type { Intent } from "../store/msg";
import { machineTitle, todayOf, windowTexts } from "../store/spend";
import { type State, currentProject } from "../store/state";
import { NavLink } from "./Link";

/// Splits "5h 48% used · resets 15:30" so its numbers are set in the mono face.
const Figures = ({ text }: { text: string }) => (
  <>
    {text.split(/(\d[\d:%]*)/).map((part, i) => (/^\d/.test(part) ? <span key={`${i}:${part}`} className="mono">{part}</span> : part))}
  </>
);

/// The line of the window's top bar: what the open project cost today, then the plan windows when
/// the daemon has a report younger than an hour. It links to the Usage page; its title carries the
/// machine-wide figure and the estimate note. Nothing until the daemon has answered.
export function SpendLine({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const project = currentProject(state);
  const today = todayOf(state, project?.key);
  if (!today) return null;
  const windows = windowTexts(today);
  return (
    <NavLink route={{ name: "usage" }} dispatch={dispatch} className="spend-line" testId="spend-line" title={machineTitle(today, ESTIMATE_NOTE)}>
      <span data-testid="spend-today">est. <span className="mono">{costText(today.project.cost_usd)}</span> today</span>
      {windows.map((w) => (
        <span key={w} data-testid="spend-window"><Figures text={w} /></span>
      ))}
    </NavLink>
  );
}
