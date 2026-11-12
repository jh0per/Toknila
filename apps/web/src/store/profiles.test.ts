import { describe, expect, it } from "vitest";
import { isSimulated, listProfile, profileLabel, runsSimulated, showAgentName, simLast, simulatedBanner, statusWord } from "./profiles";

describe("the simulated agent", () => {
  it("is named as simulated and listed last in the Default agent select", () => {
    expect(profileLabel("sim")).toBe("Simulated (no real agent, for tests)");
    expect(profileLabel("claude-code")).toBe("claude-code");
    expect(simLast(["sim", "claude-code", "other"])).toEqual(["claude-code", "other", "sim"]);
  });

  it("shows the banner for a project whose default agent is sim, and for no other", () => {
    expect(simulatedBanner({ default_profile: "sim" })).toBe(
      "This project runs a simulated agent: cards run without a real agent. Change it in Settings, Agents.",
    );
    expect(simulatedBanner({ default_profile: "claude-code" })).toBeUndefined();
    expect(simulatedBanner(undefined)).toBeUndefined();
  });

  it("reads simulated, never plain running, for a card under sim", () => {
    expect(runsSimulated(null, "sim")).toBe(true);
    expect(runsSimulated("claude-code", "sim")).toBe(false);
    expect(runsSimulated("sim", "claude-code")).toBe(true);
    expect(statusWord("running", true)).toBe("simulated");
    expect(statusWord("running", false)).toBe("running");
    expect(statusWord("review", true)).toBe("review");
  });
});

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
