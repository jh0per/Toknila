import { describe, expect, it } from "vitest";
import type { Design } from "../api/types";
import {
  dayText, designOf, dragRegion, fileUrl, kindLabel, looksLikeDesign, placeholder, refusedFiles, regionStyle, regionText,
  isImage, sizeText, stepZoom, thumbUrl, usedText, withRef,
} from "./designs";

const design = (over: Partial<Design>): Design => ({
  id: "01M748YXDZ78RYHNWGEQFCX3MQ", name: "home.png", kind: "image", mime: "image/png", size: 10,
  created_at: 0, figma: null, has_file: true, thumb: true, hint: null, ...over,
});

describe("designs", () => {
  it("accepts png, jpg, webp and pdf by type or by name, nothing else", () => {
    expect(looksLikeDesign({ name: "a", type: "image/png" })).toBe(true);
    expect(looksLikeDesign({ name: "A.JPEG", type: "" })).toBe(true);
    expect(looksLikeDesign({ name: "a.pdf", type: "application/pdf" })).toBe(true);
    expect(looksLikeDesign({ name: "a.svg", type: "image/svg+xml" })).toBe(false);
    expect(looksLikeDesign({ name: "notes.txt", type: "text/plain" })).toBe(false);
  });

  it("names the files of a drop that cannot be designs and the ones that are too large", () => {
    expect(refusedFiles([{ name: "a.png", type: "image/png", size: 1 }])).toBe("");
    expect(refusedFiles([{ name: "a.txt", type: "text/plain", size: 1 }, { name: "b.png", type: "image/png", size: 1 }]))
      .toBe("a.txt: only png, jpg, webp and pdf files can be designs");
    expect(refusedFiles([{ name: "big.png", type: "image/png", size: 26 * 1024 * 1024 }])).toBe("big.png: larger than 25 MB");
  });

  it("labels kind, size and day", () => {
    expect(kindLabel(design({}))).toBe("PNG");
    expect(kindLabel(design({ kind: "pdf", mime: "application/pdf" }))).toBe("PDF");
    expect(kindLabel(design({ kind: "figma", mime: null }))).toBe("Figma");
    expect(sizeText(512)).toBe("512 B");
    expect(sizeText(2048)).toBe("2 KB");
    expect(sizeText(1.5 * 1024 * 1024)).toBe("1.5 MB");
    expect(dayText(Date.UTC(2026, 10, 11, 12))).toBe("2026-11-11");
  });

  it("serves the file with the token in the address, encoded", () => {
    expect(fileUrl("DEMO", design({}), "a b")).toBe("/v1/projects/DEMO/designs/01M748YXDZ78RYHNWGEQFCX3MQ/file?token=a%20b");
  });

  it("shows the export hint for a link without an image and a mark for a pdf", () => {
    expect(placeholder(design({ kind: "figma", hint: "Export the frame" }))).toBe("Export the frame");
    expect(placeholder(design({ kind: "figma", hint: null }))).toContain("Export the frame");
    expect(placeholder(design({ kind: "pdf" }))).toBe("PDF");
    expect(placeholder(design({}))).toBe("");
  });
});

describe("card design references", () => {
  it("draws a region from two corners as whole percent inside the picture", () => {
    expect(dragRegion({ x: 0.2, y: 0.1 }, { x: 0.6, y: 0.4 })).toEqual({ x: 20, y: 10, w: 40, h: 30 });
    // Dragged the other way round, and past the edges: the same box, clamped.
    expect(dragRegion({ x: 0.6, y: 0.4 }, { x: 0.2, y: 0.1 })).toEqual({ x: 20, y: 10, w: 40, h: 30 });
    expect(dragRegion({ x: -0.5, y: 0.5 }, { x: 2, y: 0.9 })).toEqual({ x: 0, y: 50, w: 100, h: 40 });
    // Control: a click that did not move is no region.
    expect(dragRegion({ x: 0.3, y: 0.3 }, { x: 0.3, y: 0.3 })).toBeUndefined();
  });

  it("outlines a region in percent of the picture and says it in words", () => {
    const r = { x: 20, y: 10, w: 40, h: 30 };
    expect(regionStyle(r)).toEqual({ left: "20%", top: "10%", width: "40%", height: "30%" });
    expect(regionText(r)).toBe("x 20%, y 10%, 40% wide, 30% tall");
  });

  it("steps the zoom up and down and stops at the ends", () => {
    expect(stepZoom(100, 1)).toBe(150);
    expect(stepZoom(100, -1)).toBe(75);
    expect(stepZoom(400, 1)).toBe(400);
    expect(stepZoom(50, -1)).toBe(50);
  });

  it("adds, replaces and removes a reference without touching the others", () => {
    const a = { design: "a" };
    const b = { design: "b", region: { x: 1, y: 2, w: 3, h: 4 } };
    expect(withRef([a], 1, b)).toEqual([a, b]);
    expect(withRef([a, b], 0, { design: "c" })).toEqual([{ design: "c" }, b]);
    expect(withRef([a, b], 0, undefined)).toEqual([b]);
    const list = [a];
    withRef(list, 0, undefined);
    expect(list).toEqual([a]);
  });

  it("asks what still uses a design in the words of the window", () => {
    expect(usedText({ cards: ["TOKNILA-12", "TOKNILA-14"], pins: 2 })).toBe("Used by TOKNILA-12, TOKNILA-14 and 2 pins. Remove anyway?");
    expect(usedText({ cards: ["TOKNILA-12"], pins: 0 })).toBe("Used by TOKNILA-12. Remove anyway?");
    expect(usedText({ cards: [], pins: 1 })).toBe("Used by 1 pin. Remove anyway?");
    expect(usedText({ cards: ["A-1"], pins: 1 })).toBe("Used by A-1 and 1 pin. Remove anyway?");
  });

  it("serves the picture of an image and of a pdf's first page from the thumb address", () => {
    const pdf = design({ kind: "pdf", mime: "application/pdf", thumb: true });
    expect(thumbUrl("DEMO", pdf, "t k")).toBe(`/v1/projects/DEMO/designs/${pdf.id}/thumb?token=t%20k`);
    // A pdf's first page is a thumbnail only: the page shows the pdf itself large.
    expect(isImage(pdf)).toBe(false);
    expect(isImage(design({}))).toBe(true);
    expect(isImage(design({ kind: "pdf", thumb: false }))).toBe(false);
  });

  it("finds the design of a reference, and none when the library lost it", () => {
    const d = design({});
    expect(designOf([d], { design: d.id })).toBe(d);
    expect(designOf([d], { design: "gone" })).toBeUndefined();
  });
});
