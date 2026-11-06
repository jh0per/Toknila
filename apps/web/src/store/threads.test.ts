import { describe, expect, it } from "vitest";
import type { ThreadDetail } from "../api/types";
import { blocksHandIn, shownVersion, threadStatus } from "./threads";

function thread(over: Partial<ThreadDetail> & { state?: "open" | "approved" } = {}): ThreadDetail {
  const { state = "open", ...rest } = over;
  return {
    thread: { id: "th1", number: 1, kind: "design", title: "List", state, ticket_id: "t1" },
    messages: [],
    versions: [],
    changes_requested: false,
    ...rest,
  } as unknown as ThreadDetail;
}
const ver = (number: number, at: number) => ({ number, created_at: at, body: "b", format: "html", author: "agent" });

describe("threadStatus", () => {
  it("is open for a fresh thread and approved after approval", () => {
    expect(threadStatus(thread())).toBe("open");
    expect(threadStatus(thread({ state: "approved" }))).toBe("approved");
  });

  it("is changes requested when the daemon says so, and approved wins", () => {
    expect(threadStatus(thread({ changes_requested: true }))).toBe("changes_requested");
    expect(threadStatus(thread({ changes_requested: false }))).toBe("open");
    expect(threadStatus(thread({ state: "approved", changes_requested: true }))).toBe("approved");
  });
});

describe("blocksHandIn", () => {
  it("counts open threads, changes requested included", () => {
    expect(blocksHandIn([thread(), thread({ state: "approved" })])).toBe(1);
    expect(blocksHandIn([thread({ state: "approved" })])).toBe(0);
  });
});

describe("shownVersion", () => {
  const t = thread({ versions: [ver(1, 1), ver(2, 2)] as never });
  it("is the latest unless one was picked", () => {
    expect(shownVersion(t, undefined)?.number).toBe(2);
    expect(shownVersion(t, 1)?.number).toBe(1);
    expect(shownVersion(t, 7)?.number).toBe(2);
    expect(shownVersion(thread(), undefined)).toBeUndefined();
  });
});
