import { describe, expect, it } from "vitest";
import type { Design, PlannerState, Project } from "../api/types";
import { attachedLine, canSend, placeOf, withAttached } from "./askAttach";
import { uploadForAsk } from "./attachEffects";
import type { Msg } from "./msg";
import { reduce } from "./reduce";
import { type State, initialState } from "./state";

const project = { id: "P1", key: "DEMO", name: "Demo", archived: false } as unknown as Project;
const running: PlannerState = { running: true, run: "R1", profile: "claude-code-planner" };
const design = (id: string, name: string) => ({ id, name, kind: "image", thumb: true }) as unknown as Design;

const start = (): State => ({ ...initialState({ name: "planner", project: "DEMO" }), projects: [project], loaded: true });
const apply = (s: State, ...msgs: Msg[]): State => msgs.reduce((state, m) => reduce(state, m)[0], s);
const png = new File(["x"], "shot.png", { type: "image/png" });
const done = (where: string, designs: Design[], message = ""): Msg => ({ type: "askAttach", act: "done", where, designs, message });

describe("attachments to a message", () => {
  it("adds an upload effect and holds the box busy until it answers", () => {
    const [s, fx] = reduce(start(), { type: "askAttach", act: "add", project: "DEMO", where: "DEMO", prefix: "DEMO", files: [png], pasted: true });
    expect(placeOf(s, "DEMO").busy).toBe(true);
    expect(fx).toEqual([{ type: "askUpload", project: "DEMO", where: "DEMO", prefix: "DEMO", files: [png], pasted: true, have: [] }]);
    // Files dropped while one upload runs wait for the next try.
    expect(reduce(s, { type: "askAttach", act: "add", project: "DEMO", where: "DEMO", prefix: "DEMO", files: [png] })[1]).toEqual([]);
    const answered = apply(s, done("DEMO", [design("D1", "DEMO-1.png")]));
    expect(placeOf(answered, "DEMO")).toMatchObject({ busy: false, refused: "", chips: [{ name: "DEMO-1.png" }] });
  });

  it("Send is off while an upload runs, and a send is refused", () => {
    const typed = apply(start(), { type: "editPlannerDraft", where: "DEMO", text: "look" });
    const busy = apply(typed, { type: "askAttach", act: "add", project: "DEMO", where: "DEMO", prefix: "DEMO", files: [png] });
    expect(canSend(typed, "DEMO", "look")).toBe(true);
    expect(canSend(busy, "DEMO", "look")).toBe(false);
    expect(reduce(busy, { type: "askPlanner", project: "DEMO" })[1]).toEqual([]);
  });

  it("names the chips in one last line and a message may be attachments only", () => {
    const chips = [design("D1", "checkout-ref.png"), design("D2", "TOKNILA-12-1.png")];
    expect(attachedLine(chips)).toBe("Attached designs: checkout-ref.png, TOKNILA-12-1.png");
    expect(withAttached(" make it so \n", chips)).toBe("make it so\nAttached designs: checkout-ref.png, TOKNILA-12-1.png");
    const s = apply(start(), { type: "plannerLoaded", project: "DEMO", state: running }, done("DEMO", chips));
    expect(canSend(s, "DEMO", "")).toBe(true);
    const [sending, fx] = reduce(s, { type: "askPlanner", project: "DEMO" });
    expect(fx).toContainEqual({
      type: "plannerCall", project: "DEMO", call: "ask", profile: "claude-code-planner",
      text: "Attached designs: checkout-ref.png, TOKNILA-12-1.png", raw: "Attached designs: checkout-ref.png, TOKNILA-12-1.png", wait: false,
    });
    // Once the text reached the planner the chips are gone; before, a failure keeps them.
    expect(placeOf(apply(sending, { type: "plannerFailed", error: "no" }), "DEMO").chips).toHaveLength(2);
    expect(placeOf(apply(sending, { type: "plannerAsked", project: "DEMO" }), "DEMO").chips).toEqual([]);
  });

  it("taking a chip off keeps the design in the library (no request) and the other chip", () => {
    const s = apply(start(), done("DEMO", [design("D1", "a.png"), design("D2", "b.png")]));
    const [off, fx] = reduce(s, { type: "askAttach", act: "remove", where: "DEMO", design: "D1" });
    expect(fx).toEqual([]);
    expect(placeOf(off, "DEMO").chips.map((c) => c.name)).toEqual(["b.png"]);
  });

  it("a message with a card chip carries the line and the chips of the panel's box", () => {
    const open = apply(
      { ...start(), route: { name: "card", project: "DEMO", chain: "plan", key: "DEMO-1" } },
      { type: "toggleAssistant" },
      { type: "editPlannerDraft", where: "DEMO", text: "move it" },
      done("DEMO", [design("D1", "DEMO-1-1.png")]),
    );
    const [, fx] = reduce(open, { type: "askPlanner", project: "DEMO" });
    expect(fx).toContainEqual(expect.objectContaining({
      call: "discuss", card: "DEMO-1", draft: "DEMO", text: "move it\nAttached designs: DEMO-1-1.png",
    }));
  });

  it("a refused file gives the library's sentence and the typed text stays", async () => {
    const txt = new File(["x"], "notes.txt", { type: "text/plain" });
    const typed = apply(start(), { type: "editPlannerDraft", where: "DEMO", text: "keep me" });
    const answer = await uploadForAsk({ type: "askUpload", project: "DEMO", where: "DEMO", prefix: "DEMO", files: [txt], have: [] });
    expect(answer).toMatchObject({ act: "done", designs: [], message: "notes.txt: only png, jpg, webp and pdf files can be designs" });
    const s = apply(typed, { type: "askAttach", act: "add", project: "DEMO", where: "DEMO", prefix: "DEMO", files: [txt] }, answer);
    expect(placeOf(s, "DEMO")).toMatchObject({ busy: false, chips: [], refused: "notes.txt: only png, jpg, webp and pdf files can be designs" });
    expect(s.planner.drafts.DEMO).toBe("keep me");
  });

  it("a file over 25 MB is refused with the library's words", async () => {
    const big = { name: "big.png", type: "image/png", size: 26 * 1024 * 1024 } as File;
    const answer = await uploadForAsk({ type: "askUpload", project: "DEMO", where: "DEMO", prefix: "DEMO", files: [big], have: [] });
    expect(answer).toMatchObject({ designs: [], message: "big.png: larger than 25 MB" });
  });
});
