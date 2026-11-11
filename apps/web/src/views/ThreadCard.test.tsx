import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ThreadDetail } from "../api/types";
import { ThreadCard } from "./ThreadCard";

function detail(versions: unknown[]): ThreadDetail {
  return {
    thread: { id: "th1", number: 1, kind: "design", title: "List", state: "open", ticket_id: "t1" },
    messages: [],
    versions,
    changes_requested: false,
  } as unknown as ThreadDetail;
}
const render = (d: ThreadDetail) =>
  renderToStaticMarkup(<ThreadCard ticket="DEMO-1" detail={d} picked={undefined} dispatch={() => {}} />);

describe("ThreadCard", () => {
  it("shows a thread with no version as a muted draft that cannot be approved", () => {
    const html = render(detail([]));
    expect(html).toContain('data-status="draft"');
    expect(html).toMatch(/class="thr draft"/);
    expect(html).toMatch(/data-testid="thread-state">draft</);
    expect(html).toContain("No version yet");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Approve/);
  });

  it("shows a thread with a version as open, the control", () => {
    const v = { id: "v1", number: 1, body: "graph TD", format: "mermaid", author: "agent", created_at: 1 };
    const html = render(detail([v]));
    expect(html).toContain('data-status="open"');
    expect(html).not.toContain("No version yet");
    expect(html).not.toMatch(/<button[^>]*disabled=""[^>]*>Approve/);
  });
});
