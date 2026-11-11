import { describe, expect, it } from "vitest";
import type { Design } from "../api/types";
import { dayText, fileUrl, kindLabel, looksLikeDesign, placeholder, refusedFiles, sizeText } from "./designs";

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
