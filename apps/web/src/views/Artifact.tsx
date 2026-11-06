import type { ThreadVersion } from "../api/types";
import { Mermaid } from "./Mermaid";

/// One version of a thread's artifact. HTML is untrusted (an agent wrote it): it runs in an
/// iframe with an empty `sandbox`, so no scripts, no same-origin access, no forms, no popups.
export function Artifact({ version }: { version: ThreadVersion }) {
  switch (version.format) {
    case "mermaid":
      return <Mermaid source={version.body} />;
    case "html":
      return (
        <iframe
          className="art-html"
          data-testid="mockup"
          title={`Version ${version.number}`}
          sandbox=""
          srcDoc={version.body}
        />
      );
    default:
      return <pre className="art-text" data-testid="art-text">{version.body}</pre>;
  }
}
