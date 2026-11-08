/// A one-time step for the person. Claude Code asks whether to trust a folder the first time it
/// starts there. Workspaces are git worktrees, and Claude Code keys the trust it saves on the main
/// checkout of the repository, so trusting the project's folder once covers every workspace.
/// Toknila never edits Claude Code's own configuration to do this for you.
///
/// The system checks do not say whether a folder is trusted, so this is one line with a "How to"
/// to open, not a paragraph; if a check ever says the folder is not trusted, show it open.
export function TrustNote() {
  return (
    <div className="trust-note muted" data-testid="trust-note">
      <p>Claude Code asks once to trust a project folder; until you answer, its cards wait on that question.</p>
      <details className="fold inline">
        <summary>How to</summary>
        <p>
          Open <code>claude</code> in your project&apos;s own folder (not in a workspace) and accept its &ldquo;trust this
          folder&rdquo; question. Claude Code keeps that answer for the folder and for every workspace made from it, so
          cards will not stop on that question again. Toknila never changes Claude Code&apos;s configuration files for you.
        </p>
      </details>
    </div>
  );
}
