import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MachineView } from "./MachineSettings";

const render = (max: number | undefined, busy = false, error?: string) =>
  renderToStaticMarkup(<MachineView max={max} busy={busy} error={error} save={() => {}} />);

describe("the Machine page", () => {
  it("shows the number of checks at once, and fewer is off at 1", () => {
    const html = render(1);
    expect(html).toContain("Checks at once");
    expect(html).toMatch(/data-testid="checks-limit"[^>]*>1</);
    expect(html).toMatch(/data-testid="checks-fewer"[^>]*disabled/);
    expect(html).not.toMatch(/data-testid="checks-more"[^>]*disabled/);
  });

  it("at 3 both steppers work", () => {
    const html = render(3);
    expect(html).toMatch(/data-testid="checks-limit"[^>]*>3</);
    expect(html).not.toMatch(/data-testid="checks-fewer"[^>]*disabled/);
  });

  it("says the daemon's refusal and waits for the number before it lets one change it", () => {
    expect(render(2, false, "no good")).toContain("no good");
    expect(render(undefined)).toMatch(/data-testid="checks-more"[^>]*disabled/);
  });
});
