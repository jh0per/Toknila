import { afterEach, describe, expect, it, vi } from "vitest";
import { moveDesign } from "./designEffects";

describe("moving a design reference", () => {
  afterEach(() => vi.unstubAllGlobals());

  function daemon(status: number) {
    const seen: { call: string; body: string }[] = [];
    vi.stubGlobal("window", { location: { search: "?token=t" } });
    vi.stubGlobal("fetch", async (url: string, init: RequestInit) => {
      seen.push({ call: `${init.method} ${url}`, body: String(init.body) });
      const body = status === 200 ? { ticket: { key: "T-1" } } : { error: "T-9 is not a card", code: "not_found" };
      return new Response(JSON.stringify(body), { status });
    });
    return seen;
  }

  it("is one POST to the card's move route with the target", async () => {
    const seen = daemon(200);
    await moveDesign("T-1", 2, "T-3");
    expect(seen).toEqual([{ call: "POST /v1/tickets/T-1/designs/2/move", body: '{"target":"T-3"}' }]);
  });

  it("is still one request when refused: nothing is sent to patch the cards afterwards", async () => {
    const seen = daemon(404);
    await expect(moveDesign("T-1", 0, "T-9")).rejects.toThrow("T-9 is not a card");
    expect(seen).toHaveLength(1);
  });
});
