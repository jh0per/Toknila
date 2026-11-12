import type { Project } from "../api/types";
import { simulatedBanner } from "../store/profiles";

/// The notice of a project whose default agent is the simulated one; nothing for any other.
export function SimulatedBanner({ project }: { project: Project | undefined }) {
  const text = simulatedBanner(project);
  return text ? <p className="notice" role="status" data-testid="simulated-banner">{text}</p> : null;
}
