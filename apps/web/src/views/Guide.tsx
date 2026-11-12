import { showMe } from "../store/guide";
import { GUIDE_INTRO, GUIDE_SECTIONS, type GuideSection } from "../store/guideText";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import type { Theme } from "../store/theme";
import { NavLink } from "./Link";
import { ThemeButton } from "./ThemeButton";

/// The pictures of the sections, taken from the real window by `cargo xtask guide-shots` and
/// committed under `src/assets/guide`; the bundle serves them, nothing comes from the network.
const PICTURES = import.meta.glob("../assets/guide/*.png", { eager: true, query: "?url", import: "default" }) as Record<string, string>;
const picture = (id: string, scheme: "light" | "dark"): string | undefined => PICTURES[`../assets/guide/${id}-${scheme}.png`];

/// A section's picture in the theme the window shows: the forced one, or the system's.
function Picture({ section, theme }: { section: GuideSection; theme: Theme }) {
  const light = picture(section.id, "light");
  const dark = picture(section.id, "dark");
  if (!light || !dark) return null;
  const img = (src: string) => <img src={src} alt={section.alt} data-testid={`guide-picture-${section.id}`} />;
  return (
    <figure className="guide-pic">
      {theme === "system" ? (
        <picture>
          <source media="(prefers-color-scheme: dark)" srcSet={dark} />
          {img(light)}
        </picture>
      ) : img(theme === "dark" ? dark : light)}
    </figure>
  );
}

/// The "Show me" link of a section: a link to that place, a button that opens a dialog, or, when the
/// person has no project, a link that is off and says why.
function ShowMe({ section, state, dispatch }: { section: GuideSection; state: State; dispatch: (i: Intent) => void }) {
  const to = showMe(state, section.show);
  const testId = `guide-show-${section.id}`;
  if ("off" in to) {
    const why = `${testId}-why`;
    return (
      <div className="guide-show">
        <a className="btn" role="link" aria-disabled="true" aria-describedby={why} data-testid={testId} data-off>Show me</a>
        <span className="muted" id={why} data-testid={`${testId}-why`}>{to.off}</span>
      </div>
    );
  }
  return (
    <div className="guide-show">
      {"route" in to ? (
        <NavLink route={to.route} dispatch={dispatch} className="btn" testId={testId}>Show me</NavLink>
      ) : (
        <button type="button" className="btn" data-testid={testId} onClick={() => dispatch({ type: "openDialog", dialog: to.dialog })}>Show me</button>
      )}
      <span className="muted">{section.showLabel}</span>
    </div>
  );
}

/// The Guide: how to work with Toknila, from a folder to a release. The words are data
/// (`store/guideText.ts`); this draws them.
export function Guide({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  return (
    <div className="guide" data-testid="guide">
      <header className="guide-top">
        <h1>Guide</h1>
        <p className="muted">{GUIDE_INTRO}</p>
        <ol className="guide-toc" aria-label="Sections" data-testid="guide-toc">
          {GUIDE_SECTIONS.map((s, i) => (
            <li key={s.id}><a href={`#${s.id}`} onClick={(e) => { e.preventDefault(); document.getElementById(s.id)?.scrollIntoView({ block: "start" }); }}><span className="n">{i + 1}</span>{s.title}</a></li>
          ))}
        </ol>
        <div><ThemeButton theme={state.theme} dispatch={dispatch} /></div>
      </header>
      {GUIDE_SECTIONS.map((s, i) => (
        <section key={s.id} id={s.id} className="guide-section" data-testid="guide-section" data-section={s.id}>
          <div className="guide-text">
            <h2><span className="n">{i + 1}</span>{s.title}</h2>
            <p>{s.text}</p>
            <ShowMe section={s} state={state} dispatch={dispatch} />
          </div>
          <Picture section={s} theme={state.theme} />
        </section>
      ))}
    </div>
  );
}
