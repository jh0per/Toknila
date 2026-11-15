// What the chat and the ask boxes outside it say about the assistant's turn, rendered from a
// window's state: the chat's note (a turn on, a turn that ended without an answer, a message that
// never reached the assistant) and an empty epic's box right after a send.

import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ConversationMessage, EventRecord, PlannerState, Project } from "../api/types";
import { ANSWER_WAIT_MS } from "../store/discussion";
import { type State, initialState } from "../store/state";
import { AskBoxOutside } from "./AskSent";
import { AssistantButton } from "./AssistantButton";
import { ChatPane } from "./ChatPane";
import { SidebarProject } from "./SidebarProject";

const PROJECT = { id: "P1", key: "WEB", name: "Web", repo_path: "/r" } as unknown as Project;
/// When the person's message was sent.
const SENT = 1_700_000_000_000;
const plan: ConversationMessage = { id: "m1", author: "person", text: "plan", at: SENT };
const STEP = "Reading apps/web/src/views/ChatPane.tsx";

/// The journal entry of the assistant's turn starting or ending at `at`.
const turnEntry = (seq: number, working: boolean, at: number): EventRecord =>
  ({ seq, entity: "planner", kind: "turn", entity_id: "P1", at, actor: "agent", payload: { project: "P1", working } }) as unknown as EventRecord;

function clockAt(now: number) {
  vi.useFakeTimers();
  vi.setSystemTime(now);
  vi.stubGlobal("window", { location: { search: "" }, localStorage: { getItem: () => null, setItem: () => {} } });
}

function world(o: { messages?: ConversationMessage[]; planner?: Partial<PlannerState>; events?: EventRecord[]; from?: string; sending?: boolean; draft?: string }): State {
  const s = initialState({ name: "projectChain", project: "WEB", chain: "hide" } as never);
  return {
    ...s,
    projects: [PROJECT],
    events: o.events ?? [],
    conversation: { WEB: { messages: o.messages ?? [], more: false, older: false } },
    planner: {
      ...s.planner,
      byProject: { WEB: { profile: "fake", running: true, working: false, step: null, since: null, ...o.planner } },
      from: o.from ? { WEB: o.from } : {},
      sending: o.sending ? "WEB" : undefined,
      drafts: { WEB: o.draft ?? "" },
    },
  };
}

const text = (markup: string) => markup.replace(/<[^>]+>/g, "");
const chat = (s: State) => renderToStaticMarkup(<ChatPane project={PROJECT} state={s} dispatch={() => {}} />);
const box = (s: State) =>
  renderToStaticMarkup(
    <AskBoxOutside
      project="WEB" state={s} dispatch={() => {}} testId="chain-ask" place="chain:hide" label="What should this epic do?"
      placeholder="For example: the stories for hide" button="Plan it with the assistant"
    />,
  );

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("the chat's note about the turn", () => {
  it("a turn on past 90 seconds: the waiting line with the step, and no Nudge", () => {
    clockAt(SENT + ANSWER_WAIT_MS + 60_000);
    const out = chat(world({ messages: [plan], planner: { working: true, step: STEP, since: SENT + 400 }, events: [turnEntry(1, true, SENT + 400)] }));
    expect(text(out)).toContain(`The assistant is answering… 2 min · ${STEP}`);
    expect(out).not.toContain("chat-nudge");
    expect(out).not.toContain("chat-ended");
  });

  it("a turn that ended without an answer: says so at once and offers Nudge", () => {
    clockAt(SENT + 3_000);
    const out = chat(world({ messages: [plan], events: [turnEntry(1, true, SENT + 400), turnEntry(2, false, SENT + 2_500)] }));
    expect(out).toMatch(/data-testid="chat-ended"[^>]*>[\s\S]*The assistant stopped without answering\./);
    expect(out).toMatch(/data-testid="chat-nudge"/);
    expect(out).not.toContain("chat-waiting");
  });

  it("a message no turn started for: the waiting line, and Nudge only after 90 seconds", () => {
    const events = [turnEntry(1, false, SENT - 60_000)];
    clockAt(SENT + ANSWER_WAIT_MS - 1_000);
    const early = chat(world({ messages: [plan], events }));
    expect(early).toContain("chat-waiting");
    expect(early).not.toContain("chat-nudge");
    vi.useRealTimers();
    clockAt(SENT + ANSWER_WAIT_MS + 1_000);
    const late = chat(world({ messages: [plan], events }));
    expect(late).toContain("chat-waiting");
    expect(late).toMatch(/data-testid="chat-late"[\s\S]*data-testid="chat-nudge"/);
    expect(late).not.toContain("chat-ended");
  });
});

describe("the Assistant button and the sidebar's Assistant row", () => {
  const button = (s: State) => renderToStaticMarkup(<AssistantButton state={s} dispatch={() => {}} />);
  const row = (s: State) =>
    renderToStaticMarkup(<SidebarProject project={PROJECT} state={s} dispatch={() => {}} onScreen onMenu={() => {}} onChainMenu={() => () => {}} />);

  it("pulse while the project's assistant has a turn on, with the step in their title", () => {
    clockAt(SENT);
    const s = world({ planner: { working: true, step: STEP, since: SENT } });
    expect(button(s)).toMatch(/<button[^>]*title="The assistant is working: Reading apps\/web\/src\/views\/ChatPane.tsx. Show or hide it \(Ctrl or Cmd and .\)"/);
    expect(button(s)).toMatch(/class="dot pulse"[^>]*data-testid="assistant-working"/);
    expect(row(s)).toMatch(/class="dot pulse" role="img" aria-label="The assistant is working: Reading apps\/web\/src\/views\/ChatPane.tsx"[^>]*data-testid="planner-working"/);
  });

  it("are still when no turn is on", () => {
    clockAt(SENT);
    const s = world({});
    expect(button(s)).not.toContain("pulse");
    expect(button(s)).toContain('title="Show or hide the assistant (Ctrl or Cmd and .)"');
    expect(row(s)).not.toContain("planner-working");
  });
});

describe("an empty epic's box after a send", () => {
  it("while the message is on its way: what was sent and the waiting line, not the text box", () => {
    clockAt(SENT);
    const out = box(world({ from: "chain:hide", sending: true, draft: "plan" }));
    expect(out).toMatch(/data-testid="chain-ask-sent-text"[^>]*>plan</);
    expect(text(out)).toContain("Sent to the assistant");
    expect(out).toContain('data-testid="chain-ask-waiting"');
    expect(out).not.toContain('data-testid="chain-ask-input"');
  });

  it("while the turn is on: the sent text, the waiting line with the step, and a link to the chat", () => {
    clockAt(SENT + 61_000);
    const out = box(world({ from: "chain:hide", messages: [plan], planner: { working: true, step: STEP, since: SENT + 400 }, events: [turnEntry(1, true, SENT + 400)] }));
    expect(out).toMatch(/data-testid="chain-ask-sent-text"[^>]*>plan</);
    expect(text(out)).toContain(`The assistant is answering… 1 min · ${STEP}`);
    expect(out).toMatch(/data-testid="chain-ask-open-chat"[^>]*>Open the chat</);
    expect(out).not.toContain('data-testid="chain-ask-input"');
    expect(out).not.toContain("chain-ask-nudge");
  });

  it("when the turn ended without an answer: the sent text, the stopped line with Nudge, and the box back", () => {
    clockAt(SENT + 3_000);
    const out = box(world({ from: "chain:hide", messages: [plan], events: [turnEntry(1, true, SENT + 400), turnEntry(2, false, SENT + 2_500)] }));
    expect(out).toMatch(/data-testid="chain-ask-sent-text"[^>]*>plan</);
    expect(out).toMatch(/data-testid="chain-ask-ended"[^>]*>[\s\S]*The assistant stopped without answering\./);
    expect(out).toContain('data-testid="chain-ask-nudge"');
    expect(out).toContain('data-testid="chain-ask-open-chat"');
    expect(out).toContain('data-testid="chain-ask-input"');
    expect(out).not.toContain("chain-ask-waiting");
  });

  it("after the answer: the box as it was", () => {
    clockAt(SENT + 3_000);
    const answer: ConversationMessage = { id: "m2", author: "assistant", text: "Which part?", at: SENT + 2_000 };
    const out = box(world({ from: "chain:hide", messages: [plan, answer], events: [turnEntry(1, true, SENT + 400), turnEntry(2, false, SENT + 2_500)] }));
    expect(out).toContain('data-testid="chain-ask-input"');
    expect(out).not.toContain("chain-ask-sent");
  });

  it("a message sent from the chat does not fill the box", () => {
    clockAt(SENT + 61_000);
    const out = box(world({ messages: [plan], planner: { working: true, step: STEP, since: SENT + 400 } }));
    expect(out).toContain('data-testid="chain-ask-input"');
    expect(out).not.toContain("chain-ask-sent");
  });

  it("a send that failed: the box as it was, with the text kept", () => {
    clockAt(SENT + 3_000);
    const out = box(world({ from: "chain:hide", messages: [plan], planner: { running: false }, draft: "plan" }));
    expect(out).toMatch(/data-testid="chain-ask-input"[^>]*>plan</);
    expect(out).not.toContain("chain-ask-sent");
  });
});
