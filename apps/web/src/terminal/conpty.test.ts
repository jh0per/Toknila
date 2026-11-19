import { describe, expect, it } from "vitest";
import { isWindows, ptyOptions } from "./conpty";

describe("ptyOptions", () => {
  it("names ConPTY on Windows", () => {
    expect(ptyOptions("Win32")).toEqual({ windowsPty: { backend: "conpty" } });
    expect(isWindows("Windows")).toBe(true);
  });

  it("sets nothing on a Mac or Linux, whose ptys pass bytes as they are", () => {
    for (const p of ["MacIntel", "Linux x86_64", "darwin", ""]) expect(ptyOptions(p)).toEqual({});
  });
});
