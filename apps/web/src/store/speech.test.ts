import { describe, expect, it } from "vitest";
import {
  DENIED, DICTATION_OFF, ENDED_AT_ONCE, PAUSE_MS, fieldText, hear, micFailure, settledText, shouldAutoSend, speechLang, startHeard,
} from "./speech";

describe("transcript merging", () => {
  it("keeps what was typed and appends final then interim words", () => {
    const h = hear(startHeard("Build"), [{ text: "an export", final: true }, { text: " to csv", final: false }]);
    expect(fieldText(h)).toBe("Build an export to csv");
    expect(settledText(h)).toBe("Build an export");
    expect(h.interim).toBe("to csv");
  });
  it("starts from an empty box without a leading space", () => {
    expect(fieldText(hear(startHeard(""), [{ text: "hello", final: false }]))).toBe("hello");
  });
  it("replaces the interim tail on the next result instead of piling up", () => {
    const first = hear(startHeard("a"), [{ text: "b", final: false }]);
    const second = hear(first, [{ text: "b c", final: false }]);
    expect(fieldText(second)).toBe("a b c");
  });
  it("keeps a newline the person typed", () => {
    expect(fieldText(hear(startHeard("one\n"), [{ text: "two", final: true }]))).toBe("one\ntwo");
  });
  it("has no tail when everything is final", () => {
    expect(hear(startHeard("x"), [{ text: "y", final: true }]).interim).toBe("");
  });
  it("handles Ukrainian text the same way", () => {
    expect(fieldText(hear(startHeard(""), [{ text: "створи два релізи", final: true }]))).toBe("створи два релізи");
  });
});

describe("auto-send rule", () => {
  it("sends after the pause when on and there are words", () => {
    expect(shouldAutoSend(true, "go", 1000, 1000 + PAUSE_MS)).toBe(true);
  });
  it("waits before the pause is over", () => {
    expect(shouldAutoSend(true, "go", 1000, 1000 + PAUSE_MS - 1)).toBe(false);
  });
  it("never sends when off or when the box is empty", () => {
    expect(shouldAutoSend(false, "go", 0, 10_000)).toBe(false);
    expect(shouldAutoSend(true, "  ", 0, 10_000)).toBe(false);
  });
});

describe("language and failures", () => {
  it("takes the window's language, then the first of the list, then English", () => {
    expect(speechLang({ language: "uk-UA" })).toBe("uk-UA");
    expect(speechLang({ languages: ["uk"] })).toBe("uk");
    expect(speechLang({})).toBe("en-US");
  });
  it("names what to allow and where on a refusal", () => {
    expect(micFailure("not-allowed")).toEqual({ mic: "denied", message: DENIED });
    expect(DENIED).toContain("System Settings");
  });
  it("says Dictation is off when the system's reason names it", () => {
    const f = micFailure("aborted", "Siri and Dictation are disabled");
    expect(f.mic).toBe("error");
    expect(f.message).toBe(DICTATION_OFF);
    expect(f.message).toContain("System Settings");
    expect(f.message).toContain("Dictation");
  });
  it("carries another reason of an abort in the sentence", () => {
    const f = micFailure("aborted", "Audio engine broke.");
    expect(f.mic).toBe("error");
    expect(f.message).toContain("Could not listen: Audio engine broke. Try again, or press Fn twice");
  });
  it("keeps an abort without a reason silent", () => {
    expect(micFailure("aborted")).toEqual({ mic: "idle", message: "" });
  });
  it("puts the reason of any other code in the sentence", () => {
    expect(micFailure("network", "offline").message).toContain("Could not listen: offline.");
  });
  it("says what a recognizer that ended at once means", () => {
    expect(ENDED_AT_ONCE).toContain("ended at once");
    expect(ENDED_AT_ONCE).toContain("System Settings");
  });
  it("is quiet on silence and shows other errors", () => {
    expect(micFailure("no-speech").mic).toBe("idle");
    expect(micFailure("network").mic).toBe("error");
    expect(micFailure("network").message).toContain("network");
  });
});
