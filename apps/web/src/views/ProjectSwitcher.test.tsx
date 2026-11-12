import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Project } from "../api/types";
import { initialState } from "../store/state";
import { SwitcherButton, SwitcherMenu } from "./ProjectSwitcher";

const project = (id: string, key: string, name: string) => ({ id, key, name, archived: false }) as unknown as Project;

const world = () => ({
  ...initialState({ name: "project", project: "BEA" } as never),
  projects: [project("pa", "BEA", "Beatster"), project("pb", "TOK", "Toknila"), project("pc", "LAB", "Lab notes")],
  agents: [{ project: "pb", runs: [{}], used: 1, limit: 2 }] as never,
  inbox: [{ project: "pa" }, { project: "pa" }] as never,
  switcherOpen: true,
});

const menu = (initialFilter = "", s = world()) =>
  renderToStaticMarkup(<SwitcherMenu state={s} dispatch={() => {}} initialFilter={initialFilter} />);

describe("the project switcher", () => {
  it("shows the project's badge and name with a chevron on the crumb", () => {
    const s = world();
    const h = renderToStaticMarkup(<SwitcherButton project={s.projects[0]} state={s} dispatch={() => {}} />);
    expect(h).toContain('data-testid="project-switcher"');
    expect(h).toContain('data-initials="BE"');
    expect(h).toContain("<b>Beatster</b>");
    expect(h).toContain('aria-expanded="true"');
  });

  it("lists All projects, every project and Add project, in that order", () => {
    const h = menu();
    const order = ["switcher-all", "switcher-item-BEA", "switcher-item-TOK", "switcher-item-LAB", "switcher-add"].map((id) =>
      h.indexOf(`data-testid="${id}"`));
    expect(order.every((i) => i > 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
    expect(h).toContain("All projects");
    expect(h).toContain("Add project…");
  });

  it("marks the project on the screen, a running agent and the inbox count", () => {
    const h = menu();
    const row = (key: string) => h.split(`data-testid="switcher-item-${key}"`)[1].split("</button>")[0];
    expect(row("BEA")).toContain('data-current="true"');
    expect(row("BEA")).toContain('data-testid="switcher-waiting">2<');
    expect(row("TOK")).not.toContain("data-current");
    expect(row("TOK")).toContain('data-testid="switcher-running"');
    expect(row("LAB")).not.toContain("switcher-running");
    expect(row("LAB")).not.toContain("switcher-waiting");
  });

  it("keeps the projects whose name or key has the typed text", () => {
    const h = menu("tok");
    expect(h).toContain('data-testid="switcher-item-TOK"');
    expect(h).not.toContain('data-testid="switcher-item-BEA"');
    expect(h).not.toContain('data-testid="switcher-item-LAB"');
    expect(h).toContain('data-testid="switcher-add"');
  });
});
