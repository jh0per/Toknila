import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import type { ChainDetail, Project, Suggestion } from "../api/types";
import { reduce } from "../store/reduce";
import { initialState } from "../store/state";
import { suggestionIntents } from "../store/suggestion";
import { SuggestionRow } from "./SuggestionRow";

beforeAll(() => { vi.stubGlobal("window", { location: { search: "" } }); });

const projects = [{ id: "p1", key: "DEMO", name: "Demo" }, { id: "p2", key: "OTHER", name: "Other" }] as Project[];
const chains = [{ chain: { id: "c1", name: "plan", project_id: "p1" }, tickets: [], edges: [], releases: [] }] as unknown as ChainDetail[];
const [loaded] = reduce(initialState({ name: "inbox" }), { type: "loaded", projects, chains });
const say = (extra: Partial<Suggestion>): Suggestion =>
  ({ project: "DEMO", text: "Start A", card: "DEMO-1", chain: "c1", action: "start", at: 1, ...extra }) as Suggestion;
const html = (s: Suggestion) => renderToStaticMarkup(<SuggestionRow state={loaded} suggestion={s} dispatch={() => {}} />);

describe("SuggestionRow", () => {
  it("shows the planner's text and one button for its action", () => {
    const h = html(say({}));
    expect(h).toContain(">Start A</span>");
    expect(h).toMatch(/data-testid="suggestion-button"[^>]*>Start DEMO-1<\/button>/);
    expect(h.match(/<button/g)).toHaveLength(1);
    expect(html(say({ action: "accept", card: "DEMO-2" }))).toMatch(/>Accept DEMO-2<\/button>/);
    expect(html(say({ action: "release" }))).toMatch(/>Release<\/button>/);
  });
  it("has no button when nothing waits for the person", () => {
    const h = html(say({ text: "Nothing waits for you", card: undefined, chain: undefined, action: undefined }));
    expect(h).toContain("Nothing waits for you");
    expect(h).not.toContain("<button");
  });
});

describe("what the button does", () => {
  it("starts or accepts the card, opens the card to answer, opens the release dialog of its chain", () => {
    expect(suggestionIntents(loaded, say({}))).toEqual([{ type: "ticketAction", key: "DEMO-1", action: "start" }]);
    expect(suggestionIntents(loaded, say({ action: "accept" }))).toEqual([{ type: "ticketAction", key: "DEMO-1", action: "accept" }]);
    expect(suggestionIntents(loaded, say({ action: "answer" }))).toEqual([
      { type: "navigate", route: { name: "card", project: "DEMO", chain: "plan", key: "DEMO-1" } },
    ]);
    expect(suggestionIntents(loaded, say({ action: "release" }))).toEqual([{ type: "openRelease", chain: "c1" }]);
  });
});

describe("the standing suggestion in the store", () => {
  it("is replaced by each load and reloaded on the window events that change it", () => {
    const a = say({});
    const [s1] = reduce(loaded, { type: "suggestionsLoaded", suggestions: [a] });
    expect(s1.suggestions).toEqual([a]);
    const [s2] = reduce(s1, { type: "suggestionsLoaded", suggestions: [] });
    expect(s2.suggestions).toEqual([]);
    for (const kind of ["suggest", "suggestion_cleared"]) {
      const [s, fx] = reduce({ ...loaded, lastSeq: 10 }, {
        type: "event",
        event: { seq: 11, at: 0, actor: "agent", entity: "window", entity_id: "p1", kind, payload: {} },
      });
      expect(s.lastSeq).toBe(11);
      expect(fx).toEqual([{ type: "loadInbox" }]);
    }
  });
});
