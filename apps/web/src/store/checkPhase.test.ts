import { describe, expect, it } from "vitest";
import type { ReviewView } from "../api/types";
import { checkText, elapsedText, systemRoundText } from "./checkPhase";

const view = (check?: object, check_queue?: number) =>
  ({ rounds: [], awaiting: { by: "check" }, check, check_queue }) as unknown as ReviewView;

describe("the check phase words", () => {
  it("says queued, syncing and running with its command and elapsed time", () => {
    expect(checkText(view({ phase: "queued", place: 2 }), 0)).toBe("Being checked: queued, #2 in line");
    expect(checkText(view({ phase: "syncing" }), 0)).toBe("Being checked: syncing with main");
    const running = view({ phase: "running", command: "cargo xtask verify", since: 10_000 });
    expect(checkText(running, 10_000 + 3 * 60_000 + 1)).toBe("Being checked: cargo xtask verify, 3 min");
    expect(checkText(running, 10_000 + 45_000)).toBe("Being checked: cargo xtask verify, 45 s");
  });

  it("says starting until the checker reports, and never a negative time", () => {
    expect(checkText(view(), 0)).toBe("Being checked: starting");
    expect(elapsedText(5_000, 1_000)).toBe("0 s");
  });

  it("words a round of a project with no check command as brought up to main", () => {
    expect(systemRoundText("approve", "Brought up to main; the project has no check command."))
      .toBe("brought up to main; the project has no check command");
    expect(systemRoundText("approve", "The check passed.")).toBeUndefined();
  });
});
