// Where a section's "Show me" leads in the person's own window. A pure rule over the state: the
// page only draws what it answers. Without a project there is nothing of theirs to show, so the
// link is off and says why.

import { itemRoute } from "./inbox";
import type { Dialog } from "./plan";
import type { Route } from "./route";
import { type State, chainsOf, currentProject, defaultChain } from "./state";
import { NO_PROJECT_REASON, type ShowKind } from "./guideText";

/// A place to go, a dialog to open, or nothing, with the reason.
export type ShowMe = { route: Route } | { dialog: Dialog } | { off: string };

export function showMe(s: State, kind: ShowKind): ShowMe {
  const project = currentProject(s);
  if (!project) return { off: NO_PROJECT_REASON };
  const key = project.key;
  const chain = defaultChain(s, project) ?? chainsOf(s, project)[0];
  switch (kind) {
    case "addProject": return { dialog: { kind: "addProject" } };
    case "designs": return { route: { name: "settings", project: key, section: "designs" } };
    case "assistant": return { route: { name: "planner", project: key } };
    case "chain": return { route: chain ? { name: "projectChain", project: key, chain: chain.chain.name } : { name: "project", project: key } };
    case "inbox": return { route: { name: "inbox", project: key } };
    case "review": {
      // A card of this project that waits for review; failing that, the chain it would be in.
      const waiting = s.inbox.filter((i) => i.kind === "review").map((i) => itemRoute(s, i)).find((r) => r?.name === "card" && r.project === key);
      return { route: waiting ?? (chain ? { name: "projectChain", project: key, chain: chain.chain.name } : { name: "project", project: key }) };
    }
    case "release": {
      // A chain that can be released now; failing that, the chain a release would end.
      const ready = chainsOf(s, project).find((c) => c.summary.releasable) ?? chain;
      return { route: ready ? { name: "projectChain", project: key, chain: ready.chain.name } : { name: "project", project: key } };
    }
    case "tree": return { route: { name: "project", project: key } };
  }
}
