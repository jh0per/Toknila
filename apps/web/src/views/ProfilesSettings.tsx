import type { State } from "../store/state";

/// Profiles: the built-in agent profiles and the ones from files, and the folder to put files in.
export function ProfilesSettings({ state }: { state: State }) {
  const { profiles, profilesFolder } = state.settings;
  return (
    <section className="card-block" data-testid="profiles">
      <h2>Profiles</h2>
      <p className="muted">
        A profile says how an agent starts and what it may do. A new agent is a new <code>.toml</code> file in the
        profiles folder; it applies at once.
      </p>
      <dl className="facts">
        <dt>Folder</dt>
        <dd data-testid="profiles-folder"><code>{profilesFolder || "…"}</code></dd>
      </dl>
      <ul className="profiles">
        {profiles.map((l) => (
          <li key={l.profile.name} data-profile={l.profile.name}>
            <b className="key">{l.profile.name}</b>
            <span className="muted">{l.profile.role}</span>
            <span className="muted" data-testid="profile-source">{l.file ? l.file : "built in"}</span>
            <code className="cmd">{l.profile.command.length ? l.profile.command.join(" ") : "starts no process"}</code>
          </li>
        ))}
      </ul>
    </section>
  );
}
