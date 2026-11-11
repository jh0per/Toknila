import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Design } from "../api/types";
import { CardDesignsView } from "./CardDesigns";
import { DesignPicker } from "./DesignPicker";
import { NodeThumb } from "./NodeThumb";

const design = (over: Partial<Design>): Design => ({
  id: "01M748YXDZ78RYHNWGEQFCX3MQ", name: "login.png", kind: "image", mime: "image/png", size: 10,
  created_at: 0, figma: null, has_file: true, thumb: true, hint: null, ...over,
});
const none = () => {};
beforeEach(() => {
  vi.stubGlobal("window", { location: { search: "?token=t0k" } });
  vi.stubGlobal("document", { activeElement: null });
});
const region = { x: 20, y: 10, w: 40, h: 30 };
const view = (over: Partial<Parameters<typeof CardDesignsView>[0]> = {}) =>
  renderToStaticMarkup(
    <CardDesignsView
      project="DEMO" library={[design({})]} refs={[{ design: design({}).id, region }]} canEdit={false} busy={false} onChange={none} {...over}
    />,
  );

describe("the designs on a card's page", () => {
  it("shows the mockup large from the project's file, with the region outlined in percent", () => {
    const html = view();
    expect(html).toContain('data-testid="mockup-image"');
    expect(html).toContain("/v1/projects/DEMO/designs/01M748YXDZ78RYHNWGEQFCX3MQ/file?token=");
    expect(html).toContain('data-testid="mockup-region"');
    expect(html).toContain("left:20%;top:10%;width:40%;height:30%");
    expect(html).toContain("Region: x 20%, y 10%, 40% wide, 30% tall");
  });

  it("has zoom controls, and no outline for a design shown whole", () => {
    const html = view({ refs: [{ design: design({}).id }] });
    for (const id of ["mockup-zoom-in", "mockup-zoom-out", "mockup-zoom-fit", "mockup-zoom-level"]) expect(html).toContain(`data-testid="${id}"`);
    expect(html).toContain("100%");
    expect(html).not.toContain('data-testid="mockup-region"');
  });

  it("offers Add, Change and Remove only when the card can be edited", () => {
    const off = view();
    for (const id of ["add-design", "design-edit", "design-unlink"]) expect(off).not.toContain(id);
    const on = view({ canEdit: true });
    for (const id of ["add-design", "design-edit", "design-unlink"]) expect(on).toContain(`data-testid="${id}"`);
  });

  it("says so when the library lost a design, and when the card has none", () => {
    expect(view({ library: [] })).toContain("no longer in the project");
    const empty = view({ refs: [], canEdit: true });
    expect(empty).toContain('data-testid="card-designs-none"');
    expect(empty).toContain('data-testid="add-design"');
  });

  it("shows a pdf in the page and a link without an image as its hint", () => {
    const pdf = design({ kind: "pdf", thumb: false, mime: "application/pdf" });
    expect(view({ library: [pdf], refs: [{ design: pdf.id }] })).toContain('data-testid="mockup-pdf"');
    const figma = design({ kind: "figma", thumb: false, has_file: false, hint: "Export the frame." });
    const html = view({ library: [figma], refs: [{ design: figma.id }] });
    expect(html).toContain("Export the frame.");
    expect(html).not.toContain('data-testid="mockup-image"');
  });
});

describe("the design picker", () => {
  it("lists the library and, before a choice, saves nothing", () => {
    const html = renderToStaticMarkup(
      <DesignPicker project="DEMO" library={[design({}), design({ id: "02", name: "home.png" })]} busy={false} onSave={none} onClose={none} />,
    );
    expect(html.match(/data-testid="picker-item"/g)?.length).toBe(2);
    expect(html).toMatch(/data-testid="picker-save"[^>]*disabled|disabled=""[^>]*data-testid="picker-save"/);
    expect(html).not.toContain('data-testid="picker-stage"');
  });

  it("editing a reference opens its picture with the region drawn", () => {
    const d = design({});
    const html = renderToStaticMarkup(
      <DesignPicker project="DEMO" library={[d]} initial={{ design: d.id, region }} busy={false} onSave={none} onClose={none} />,
    );
    expect(html).toContain('data-testid="picker-stage"');
    expect(html).toContain("left:20%");
    expect(html).toContain("Region: x 20%");
  });
});

describe("the thumbnail on a card of the diagram", () => {
  it("is the first design's file with its region, and counts the others", () => {
    const html = renderToStaticMarkup(<NodeThumb project="DEMO" designs={[{ design: "a", region }, { design: "b" }, { design: "c" }]} />);
    expect(html).toContain("/v1/projects/DEMO/designs/a/file?token=");
    expect(html).toContain("left:20%");
    expect(html).toContain("+2");
  });

  it("is nothing for a card without designs", () => {
    expect(renderToStaticMarkup(<NodeThumb project="DEMO" designs={[]} />)).toBe("");
  });
});
