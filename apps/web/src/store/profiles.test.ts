import { describe, expect, it } from "vitest";
import { isSimulated, listProfile, showAgentName } from "./profiles";

describe("agent names", () => {
  it("knows the simulated profiles", () => {
    expect(["sim", "sim-reviewer", "fake-claude"].every(isSimulated)).toBe(true);
    expect(["claude-code", "claude-code-reviewer", "simple"].some(isSimulated)).toBe(false);
  });

  it("shows a chip only in a development build or when it is not the project's default", () => {
    expect(showAgentName("sim", "sim", false)).toBe(false);
    expect(showAgentName("sim", "claude-code", false)).toBe(true);
    expect(showAgentName("claude-code", "claude-code", true)).toBe(true);
  });

  it("lists simulated profiles only in development or where the project uses them", () => {
    expect(listProfile("sim", "claude-code", false)).toBe(false);
    expect(listProfile("sim", "sim", false)).toBe(true);
    expect(listProfile("sim", undefined, true)).toBe(true);
    expect(listProfile("claude-code", undefined, false)).toBe(true);
  });
});
