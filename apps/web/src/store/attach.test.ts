import { afterEach, describe, expect, it, vi } from "vitest";
import { pastedName, reduceAttach } from "./attach";
import { attachFiles } from "./attachEffects";
import { initialState } from "./state";
import type { TicketDetail } from "../api/types";

const png = (name: string) => new File([new Uint8Array([1, 2, 3])], name, { type: "image/png" });
const txt = (name: string) => new File(["hi"], name, { type: "text/plain" });

describe("pastedName", () => {
  it("counts up from the first free number, in any letter case", () => {
    expect(pastedName("T-1", "image/png", [])).toBe("T-1-1.png");
    expect(pastedName("T-1", "image/png", ["t-1-1.PNG", "T-1-2.png"])).toBe("T-1-3.png");
    expect(pastedName("T-1", "image/png", [], ["T-1-1.png"])).toBe("T-1-2.png");
    expect(pastedName("T-1", "image/jpeg", [])).toBe("T-1-1.jpg");
  });
});

describe("attaching files", () => {
  afterEach(() => vi.unstubAllGlobals());

  /// A daemon with a library; `fail` names the uploads it refuses. Answers every call and records it.
  function daemon(opts: { fail?: string[]; library?: string[]; patchFails?: boolean } = {}) {
    const calls: string[] = [];
    let n = 0;
    vi.stubGlobal("window", { location: { search: "?token=t" } });
    vi.stubGlobal("fetch", async (url: string, init?: RequestInit) => {
      const method = init?.method ?? "GET";
      calls.push(`${method} ${decodeURIComponent(url)}`);
      const json = (b: unknown, status = 200) => new Response(JSON.stringify(b), { status });
      if (method === "GET") return json({ designs: (opts.library ?? []).map((name) => ({ name })) });
      if (method === "POST") {
        const name = new URL(url, "http://x").searchParams.get("name") ?? "";
        if (opts.fail?.includes(name)) return json({ error: "Only png, jpg, webp and pdf files can be designs." }, 400);
        n += 1;
        return json({ id: `D${n}`, name });
      }
      if (opts.patchFails) return json({ error: "The card is not editable." }, 409);
      return json({ ticket: { key: "T-1", designs: JSON.parse(String(init?.body)).designs } });
    });
    return calls;
  }
  const effect = (files: File[], pasted = false) => ({
    type: "attachFiles" as const, project: "DEMO", card: "T-1", files, pasted, current: [{ design: "OLD" }],
  });

  it("uploads each file and saves the card once, after the card's own designs", async () => {
    const calls = daemon();
    const r = await attachFiles(effect([png("a.png"), png("b.png")]));
    expect(calls).toEqual([
      "POST /v1/projects/DEMO/designs?name=a.png",
      "POST /v1/projects/DEMO/designs?name=b.png",
      "PATCH /v1/tickets/T-1",
    ]);
    expect(r.detail?.ticket.designs).toEqual([{ design: "OLD" }, { design: "D1" }, { design: "D2" }]);
    expect(r.message).toBe("");
  });

  it("names a pasted image from the card's key and skips the names the library has", async () => {
    const calls = daemon({ library: ["T-1-1.png"] });
    await attachFiles(effect([png("image.png"), png("image.png")], true));
    expect(calls.filter((c) => c.startsWith("POST"))).toEqual([
      "POST /v1/projects/DEMO/designs?name=T-1-2.png",
      "POST /v1/projects/DEMO/designs?name=T-1-3.png",
    ]);
  });

  it("refuses a file that cannot be a design with the library's sentence, and attaches the others", async () => {
    const calls = daemon();
    const r = await attachFiles(effect([txt("notes.txt"), png("a.png")]));
    expect(r.message).toBe("notes.txt: only png, jpg, webp and pdf files can be designs");
    expect(calls.filter((c) => c.startsWith("POST"))).toEqual(["POST /v1/projects/DEMO/designs?name=a.png"]);
    expect(r.detail?.ticket.designs).toEqual([{ design: "OLD" }, { design: "D1" }]);
  });

  it("leaves the card's designs as they were when every upload fails", async () => {
    const calls = daemon({ fail: ["a.png"] });
    const r = await attachFiles(effect([png("a.png")]));
    expect(r.detail).toBeUndefined();
    expect(r.message).toBe("a.png: Only png, jpg, webp and pdf files can be designs.");
    expect(calls.some((c) => c.startsWith("PATCH"))).toBe(false);
  });

  it("says so, and changes no state of the card, when the save is refused", async () => {
    daemon({ patchFails: true });
    const r = await attachFiles(effect([png("a.png")]));
    expect(r.detail).toBeUndefined();
    expect(r.message).toBe("The card is not editable.");
  });

  it("the reducer waits for the answer, takes no second drop meanwhile, and keeps the card's detail", () => {
    const s0 = initialState({ name: "planner", project: "DEMO" });
    const detail = { ticket: { key: "T-1", chain_id: "C1", designs: [] }, can_edit: true } as unknown as TicketDetail;
    const withCard = { ...s0, details: { "T-1": detail } };
    const [busy, fx] = reduceAttach(withCard, { type: "attachFiles", project: "DEMO", card: "T-1", files: [png("a.png")] });
    expect(busy.attach.card).toBe("T-1");
    expect(fx).toHaveLength(1);
    expect(reduceAttach(busy, { type: "attachFiles", project: "DEMO", card: "T-1", files: [png("b.png")] })[1]).toEqual([]);
    const saved = { ...detail, ticket: { ...detail.ticket, designs: [{ design: "D1" }] } } as TicketDetail;
    const [done, fx2] = reduceAttach(busy, { type: "filesAttached", card: "T-1", detail: saved, message: "x: refused" });
    expect(done.attach).toEqual({ card: undefined, message: "x: refused", of: "T-1" });
    expect(done.details["T-1"].ticket.designs).toEqual([{ design: "D1" }]);
    expect(fx2).toEqual([{ type: "loadChain", id: "C1" }]);
    const [failed] = reduceAttach(busy, { type: "filesAttached", card: "T-1", message: "boom" });
    expect(failed.details["T-1"].ticket.designs).toEqual([]);
  });
});
