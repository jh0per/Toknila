import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { reduce } from "./reduce";
import { initialState } from "./state";
import { parseTheme } from "./theme";
import { THEME_KEY, applyTheme, readTheme, saveTheme } from "./themeEffects";

describe("the theme choice", () => {
  it("is state, and remembering it is an effect", () => {
    const [s, fx] = reduce(initialState({ name: "chains" }), { type: "setTheme", theme: "dark" });
    expect(s.theme).toBe("dark");
    expect(fx).toEqual([{ type: "saveTheme", theme: "dark" }]);
  });

  it("starts on the system's colours", () => {
    expect(initialState({ name: "chains" }).theme).toBe("system");
  });

  it("reads only the three names; anything else follows the system", () => {
    expect(parseTheme("light")).toBe("light");
    expect(parseTheme("dark")).toBe("dark");
    expect(parseTheme("system")).toBe("system");
    expect(parseTheme("sepia")).toBe("system");
    expect(parseTheme(null)).toBe("system");
  });
});

/// A browser in miniature: the document element's attribute and a storage that may refuse.
function browser(storage: Pick<Storage, "getItem" | "setItem">) {
  const attrs = new Map<string, string>();
  vi.stubGlobal("document", {
    documentElement: {
      setAttribute: (k: string, v: string) => attrs.set(k, v),
      removeAttribute: (k: string) => attrs.delete(k),
    },
  });
  vi.stubGlobal("window", { localStorage: storage });
  return attrs;
}

describe("the theme on the page", () => {
  beforeEach(() => vi.unstubAllGlobals());
  afterEach(() => vi.unstubAllGlobals());

  it("forces light or dark on <html> and leaves System to the browser", () => {
    const attrs = browser({ getItem: () => null, setItem: () => undefined });
    applyTheme("dark");
    expect(attrs.get("data-theme")).toBe("dark");
    applyTheme("light");
    expect(attrs.get("data-theme")).toBe("light");
    applyTheme("system");
    expect(attrs.has("data-theme")).toBe(false);
  });

  it("remembers the choice for this viewer and reads it back", () => {
    const store = new Map<string, string>();
    browser({ getItem: (k) => store.get(k) ?? null, setItem: (k, v) => void store.set(k, v) });
    saveTheme("dark");
    expect(store.get(THEME_KEY)).toBe("dark");
    expect(readTheme()).toBe("dark");
  });

  it("still applies when storage refuses to write or to read", () => {
    const refuse = () => {
      throw new Error("blocked");
    };
    const attrs = browser({ getItem: refuse, setItem: refuse });
    saveTheme("light");
    expect(attrs.get("data-theme")).toBe("light");
    expect(readTheme()).toBe("system");
  });
});
