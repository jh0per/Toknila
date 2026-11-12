import type { ScreenAsked } from "../store/trustAsked";

/// A one-time step for the person. Claude Code asks whether to trust a folder the first time it
/// starts there. The ships of a project live under one folder, `<ships>/<project>`, so trusting
/// that folder once covers every card of the project. Toknila never edits Claude Code's own
/// configuration to do this for you, and never answers the question for you: when it comes up on
/// a card, the card waits and the question is in the Inbox.
///
/// The system checks do not say whether a folder is trusted, so this is one line with a "How to"
/// to open, not a paragraph. `folder` is the project's ships folder when it is known; `asked` is
/// the last time Claude Code asked in the project (the project's settings show it).
export function TrustNote({ folder, asked }: { folder?: string; asked?: ScreenAsked | null }) {
  return (
    <div className="trust-note muted" data-testid="trust-note">
      <p>Claude Code asks once to trust a project folder; until you answer, its stories wait on that question.</p>
      <details className="fold inline">
        <summary>How to</summary>
        <p>
          Open <code>claude</code> in the project&apos;s ships folder
          {folder ? <> (<code data-testid="trust-folder">{folder}</code>)</> : <> (the project&apos;s own folder under the workspaces folder, <code>&lt;ships&gt;/&lt;project&gt;</code>)</>}
          {" "}and accept its &ldquo;trust this folder&rdquo; question once. Claude Code keeps that answer for the folder and for every
          story&apos;s workspace under it, so stories will not stop on that question again. Or answer the question when a story shows it
          in the Inbox. Toknila never changes Claude Code&apos;s configuration files for you.
        </p>
      </details>
      {asked !== undefined && (
        <p data-testid="trust-asked">
          {asked ? `Claude Code last asked on ${new Date(asked.at).toLocaleString()}: ${asked.text}` : "Claude Code has not asked in this project yet."}
        </p>
      )}
    </div>
  );
}
