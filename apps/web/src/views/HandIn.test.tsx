import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Comment } from "../api/types";
import { HandIn, parseCheck } from "./HandIn";

const report = (body: Comment["body"]): Comment =>
  ({ id: "c", ticket_id: "t", kind: "report", author: "agent", body, created_at: 0, delivered_at: null }) as unknown as Comment;

describe("parseCheck", () => {
  it("splits a status from its evidence", () => {
    expect(parseCheck("- PASS verify: 412 tests")).toEqual({ status: "PASS", text: "verify: 412 tests" });
    expect(parseCheck("- NOT_RUN e2e: no browser")).toEqual({ status: "NOT_RUN", text: "e2e: no browser" });
    expect(parseCheck("all fine")).toBeUndefined();
  });
});

describe("HandIn", () => {
  it("shows what changed, the checks and what is left, each as its own block", () => {
    const html = renderToStaticMarkup(
      <HandIn report={report({ gist: "Template added", how_to_verify: "- PASS verify: green\n- FAIL e2e: red", consequences: "docs" })} />,
    );
    expect(html).toMatch(/data-testid="handin-changed"[^>]*>.*Template added/);
    expect(html).toMatch(/data-testid="handin-checks".*data-status="PASS".*PASS<\/span> verify: green.*data-status="FAIL"/);
    expect(html).toMatch(/data-testid="handin-left"[^>]*>.*docs/);
  });

  it("keeps an older report without checks as plain text", () => {
    const html = renderToStaticMarkup(<HandIn report={report({ gist: "Old report", what: "did it" })} />);
    expect(html).toContain("Old report");
    expect(html).not.toContain("handin-checks");
  });
});

describe("Since the Return", () => {
  const since = {
    round: 2,
    from: "f8afe06aaaaaaaa",
    to: "b0b0b0bbbbbbbbb",
    commits: ["fix login", "rename flag"],
    more_commits: 3,
    files: ["flag.txt", "login.txt"],
    more_files: 4,
  };
  const handIn = report({ gist: "Done", how_to_verify: "- PASS verify: green", consequences: "nothing" });

  it("lists the commits and the changed files under the hand-in blocks, with what was cut", () => {
    const html = renderToStaticMarkup(<HandIn report={handIn} since={since} />);
    expect(html).toMatch(/data-testid="handin-left".*data-testid="handin-since"/);
    expect(html).toContain("Since the Return");
    expect(html).toContain("Round 2, f8afe06..b0b0b0b");
    expect(html).toMatch(/<li>fix login<\/li><li>rename flag<\/li>/);
    expect(html).toContain("and 3 more commits");
    expect(html).toContain("Changed files: flag.txt, login.txt and 4 more");
  });

  it("is absent for a hand-in that answers no Return", () => {
    expect(renderToStaticMarkup(<HandIn report={handIn} />)).not.toContain("handin-since");
    expect(renderToStaticMarkup(<HandIn report={handIn} since={null} />)).not.toContain("Since the Return");
  });
});
