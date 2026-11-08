import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { PullRequest, Ship, Ticket } from "../api/types";
import { DetailsMeta, TechnicalDetails } from "./DetailsMeta";

const card = (extra: Partial<Ticket>) => ({ key: "DEMO-3", ...extra }) as Ticket;
const ship = { path: "/work/ships/demo-3", branch: "ship/demo/three", key: "DEMO-3", ticket_id: "t" } as Ship;

describe("DetailsMeta", () => {
  it("shows the out-of-order mark", () => {
    const html = renderToStaticMarkup(<DetailsMeta card={card({ done_out_of_order: { behind: ["DEMO-1"] } })} />);
    expect(html).toContain("done out of order: DEMO-1 still open");
  });
  it("links the pull request and lists its checks", () => {
    const pr = {
      number: 7,
      url: "https://github.com/o/r/pull/7",
      checks: [{ name: "verify", state: "failed" }, { name: "desktop", state: "passed" }],
    } as PullRequest;
    const html = renderToStaticMarkup(<DetailsMeta card={card({ pr })} />);
    expect(html).toContain('href="https://github.com/o/r/pull/7"');
    expect(html).toContain("verify: failed");
    expect(html).toContain("desktop: passed");
  });
  it("renders nothing for a plain card", () => {
    expect(renderToStaticMarkup(<DetailsMeta card={card({})} />)).toBe("");
  });
});

describe("TechnicalDetails", () => {
  it("is a closed fold named Technical details, holding the workspace and its branch", () => {
    const html = renderToStaticMarkup(<TechnicalDetails card={card({})} ship={ship} projectDefault="claude-code" />);
    expect(html).toMatch(/<details[^>]*>\s*<summary>Technical details<\/summary>/);
    expect(html).not.toContain("<details open");
    expect(html).toContain("/work/ships/demo-3");
    expect(html).toContain("ship/demo/three");
    expect(html).not.toContain("Worktree");
  });
  it("names the card's own agent, else the project's default", () => {
    const own = renderToStaticMarkup(<TechnicalDetails card={card({ profile: "sim" })} ship={null} projectDefault="claude-code" />);
    expect(own).toContain("sim");
    expect(own).not.toContain("the project&#x27;s default");
    const inherited = renderToStaticMarkup(<TechnicalDetails card={card({})} ship={null} projectDefault="claude-code" />);
    expect(inherited).toContain("claude-code");
    expect(inherited).toContain("the project&#x27;s default");
  });
  it("renders nothing without a workspace or an agent", () => {
    expect(renderToStaticMarkup(<TechnicalDetails card={card({})} ship={null} projectDefault={undefined} />)).toBe("");
  });
});
