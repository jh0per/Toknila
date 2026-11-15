import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { initialState } from "../store/state";
import { NewChain } from "./NewChain";
import { chainInput, slugFromTitle, slugLine } from "./newChainForm";

describe("the New epic dialog's slug", () => {
  it("follows the title as the person types", () => {
    expect(slugFromTitle("MIDI controllers")).toBe("midi-controllers");
    expect(slugFromTitle("Сховати проект зі списку")).toBe("skhovaty-proekt-zi-spysku");
    expect(slugFromTitle("  A -- B  ")).toBe("a-b");
    expect(slugFromTitle("安卓")).toBeUndefined();
    expect(slugLine({ title: "MIDI controllers" })).toEqual({ slug: "midi-controllers", edited: false });
    expect(slugLine({ title: "MIDI controllers pro" }).slug).toBe("midi-controllers-pro");
  });

  it("stays as typed once the person changes it", () => {
    expect(slugLine({ title: "Other words", typed: "my-slug" })).toEqual({ slug: "my-slug", edited: true });
  });

  it("is shown under the title in the dialog", () => {
    const html = renderToStaticMarkup(
      <NewChain project="DEMO" state={initialState({ name: "project", project: "DEMO" } as never)} dispatch={() => {}} initialTitle="MIDI controllers" />,
    );
    expect(html).toContain("becomes");
    expect(html).toContain("midi-controllers");
    expect(html).toContain('data-testid="chain-title"');
    expect(html).toContain("Change");
  });
});

describe("the request of the dialog", () => {
  const base = { goal: "", autostart: true, mode: "" as const, version: "" };

  it("carries the title and the slug", () => {
    expect(chainInput({ ...base, title: "  MIDI controllers ", slug: "midi-controllers" })).toEqual({
      title: "MIDI controllers", name: "midi-controllers", goal: null, autostart: true, review_mode: null, version: null,
    });
  });

  it("carries a slug that was typed", () => {
    expect(chainInput({ ...base, title: "MIDI controllers", slug: "midi" }).name).toBe("midi");
  });

  it("leaves the name out when the title gives no slug, so the daemon names it", () => {
    expect(chainInput({ ...base, title: "安卓", slug: undefined }).name).toBeUndefined();
  });
});
