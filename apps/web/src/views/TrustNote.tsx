/// A one-time step for the person. Claude Code asks whether to trust a folder the first time it
/// starts there. Ships are git worktrees, and Claude Code keys the trust it saves on the main
/// checkout of the repository, so trusting the project's repository once covers every ship. Toknila
/// never edits Claude Code's own configuration to do this for you.
export function TrustNote() {
  return (
    <p className="muted" data-testid="trust-note">
      One time, for Claude Code: open <code>claude</code> in your project&apos;s own repository (not a ship) and accept
      its &ldquo;trust this folder&rdquo; question. Claude Code keeps that answer for the repository and for every ship
      (worktree) made from it, so cards will not stop on that dialog. Toknila never changes Claude Code&apos;s
      configuration files for you.
    </p>
  );
}
