import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SoundsView } from "./SoundSettings";

const off = { story_done: false, epic_done: false, problem: false };

describe("the Sounds section", () => {
  it("has a switch in plain words and a Play button for each of the three sounds", () => {
    const html = renderToStaticMarkup(<SoundsView switches={off} dispatch={() => {}} />);
    for (const words of ["A story is done", "An epic is finished", "Something went wrong"]) expect(html).toContain(words);
    for (const kind of ["story_done", "epic_done", "problem"]) {
      expect(html).toContain(`data-testid="sound-${kind}"`);
      expect(html).toContain(`data-testid="play-${kind}"`);
    }
    expect(html).not.toContain("checked");
  });

  it("shows a switch that is on as on", () => {
    const html = renderToStaticMarkup(<SoundsView switches={{ ...off, epic_done: true }} dispatch={() => {}} />);
    expect(html.match(/checked/g)).toHaveLength(1);
    expect(html).toMatch(/data-testid="sound-epic_done"[^>]*checked|checked=""[^>]*data-testid="sound-epic_done"/);
  });
});
