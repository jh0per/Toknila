import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Design } from "../api/types";
import type { Pin } from "../store/pins";
import { CardDesignsView } from "./CardDesigns";
import { PinList, PinMarkers } from "./PinOverlay";

const DESIGN = "01M748YXDZ78RYHNWGEQFCX3MQ";
const design: Design = {
  id: DESIGN, name: "login.png", kind: "image", mime: "image/png", size: 10,
  created_at: 0, figma: null, has_file: true, thumb: true, hint: null,
};
const pin = (over: Partial<Pin>): Pin => ({
  id: "01PIN1", project: "p", design: DESIGN, design_name: "login.png", number: 1,
  region: { x: 12, y: 30, w: 40, h: 20 }, text: "Is the logo fixed?", card: "DEMO-1", created_at: 0, ...over,
});
const none = async () => {};

beforeEach(() => {
  vi.stubGlobal("window", { location: { search: "?token=t0k" } });
  vi.stubGlobal("document", { activeElement: null });
});

describe("the pins on a mockup", () => {
  it("draws a numbered marker at the region's corner, in percent of the picture", () => {
    const html = renderToStaticMarkup(<PinMarkers pins={[pin({}), pin({ id: "01PIN2", number: 2, region: { x: 50, y: 60, w: 10, h: 10 } })]} active={undefined} onSelect={() => {}} />);
    expect(html.match(/data-testid="pin-marker"/g)).toHaveLength(2);
    expect(html).toContain("left:12%;top:30%");
    expect(html).toContain("left:50%;top:60%");
    expect(html).toContain('data-number="2"');
    expect(html).toContain("Pin 1, open: Is the logo fixed?");
  });

  it("marks an answered pin differently and outlines the region of the selected one only", () => {
    const answered = pin({ id: "01PIN2", number: 2, answer: "Yes", region: { x: 5, y: 6, w: 7, h: 8 } });
    const html = renderToStaticMarkup(<PinMarkers pins={[pin({}), answered]} active={answered.id} onSelect={() => {}} />);
    expect(html).toContain('data-state="open"');
    expect(html).toContain('data-state="answered"');
    expect(html.match(/data-testid="pin-region"/g)).toHaveLength(1);
    expect(html).toContain("left:5%;top:6%;width:7%;height:8%");
  });

  it("lists an open pin with its question and an answer box, and an answered one with the answer", () => {
    const html = renderToStaticMarkup(
      <PinList pins={[pin({}), pin({ id: "01PIN2", number: 2, text: "Tax?", answer: "Separate line" })]} active={undefined} onSelect={() => {}} onAnswer={none} />,
    );
    expect(html).toContain("Is the logo fixed?");
    expect(html).toContain("The assistant asks");
    expect(html.match(/data-testid="pin-input"/g)).toHaveLength(1);
    expect(html).toContain('data-testid="pin-answer"');
    expect(html).toContain("Separate line");
    expect(html).toContain('data-testid="pin-edit"');
    expect(html).toContain("x 12%, y 30%, 40% wide, 20% tall");
  });

  it("shows nothing for no pins", () => {
    expect(renderToStaticMarkup(<PinList pins={[]} active={undefined} onSelect={() => {}} onAnswer={none} />)).toBe("");
  });
});

describe("pins on the card page", () => {
  const view = (pins: Pin[]) =>
    renderToStaticMarkup(
      <CardDesignsView
        project="DEMO" library={[design]} refs={[{ design: DESIGN }]} canEdit={false} busy={false} onChange={() => {}}
        cardKey="DEMO-1" pins={pins} onAnswer={none}
      />,
    );

  it("shows the pins about this card on its mockup, with their text", () => {
    const html = view([pin({}), pin({ id: "01PIN2", number: 2, card: "DEMO-2", text: "Another card's" })]);
    expect(html).toContain('data-testid="pin-marker"');
    expect(html).toContain("Is the logo fixed?");
    expect(html).not.toContain("Another card");
  });

  it("shows no marker without pins", () => {
    expect(view([])).not.toContain("pin-marker");
  });
});
