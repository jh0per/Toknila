// The words of the Guide page. This is the one place the text lives: the page draws it, so a
// section changes here without touching the layout. Words follow docs/glossary.md (epic, story,
// task, main branch, test command, assistant, hand-in). Every sentence is checked against the screens it
// describes; a screen that changes changes its section in the same change.

/// Where a section's "Show me" leads, resolved against the person's own window in `guide.ts`.
export type ShowKind = "addProject" | "designs" | "assistant" | "chain" | "inbox" | "review" | "release" | "tree";

export interface GuideSection {
  /// The anchor and the name of the section's pictures (`<id>-light.png`, `<id>-dark.png`).
  id: string;
  title: string;
  /// Two to four plain sentences, one string.
  text: string;
  /// What the picture shows, for people who cannot see it.
  alt: string;
  show: ShowKind;
  /// The label of the "Show me" link.
  showLabel: string;
}

export const GUIDE_INTRO = "From an empty folder to a release, in nine steps. The pictures are of the real window.";

/// Said beside a "Show me" that is off because the person has no project yet.
export const NO_PROJECT_REASON = "Add a project first: there is nothing of yours to show yet.";

export const GUIDE_SECTIONS: GuideSection[] = [
  {
    id: "add-project", title: "Add a project", show: "addProject", showLabel: "Add a project",
    alt: "The Add a project form with a folder, the main branch and Details open",
    text:
      "Choose the folder that holds your project, a git repository. If the folder is missing or is not a repository yet, Toknila offers to create it or set it up. " +
      "It fills in the main branch, which agents start from and where approved work lands, and under Details the test command that agents run before they hand in. " +
      "Also under Details you choose where accepted work lands: merged into the main branch, or sent as a pull request on GitHub.",
  },
  {
    id: "designs", title: "Give it designs", show: "designs", showLabel: "Open the designs",
    alt: "A project's Designs page: the library and where to drop a file or paste a Figma link",
    text:
      "Each project has a library of designs under Designs in the sidebar: png, jpg, webp or pdf files, or the link of a Figma frame. " +
      "Drop a screenshot on a story, or attach one to a message to the assistant, and the agent gets it with its work. " +
      "Stories can also ask for a design first: the agent shows one and writes code only after you approve it.",
  },
  {
    id: "plan", title: "Plan with the assistant", show: "assistant", showLabel: "Open the assistant",
    alt: "The roadmap with the assistant panel beside it and its chip, the roadmap",
    text:
      "The assistant is a smart helper for the process: it creates epics and their stories, steers the process, answers your questions, and turns a screenshot into a design story. " +
      "There is one assistant, at two levels: open Assistant under your project and the roadmap opens with it beside, where it plans epics (an epic holds stories and ends in a release); inside an epic the same panel (Cmd/Ctrl+.) talks about that epic's stories, and its chip says what is open, which you can take off. " +
      "The expand button of the panel opens the chat at full width, with the assistant's terminal.",
  },
  {
    id: "start", title: "Start a story", show: "chain", showLabel: "Open an epic",
    alt: "An epic's diagram with stories, the agents count and the Start stories automatically switch",
    text:
      "Press Start on a story, or S on the diagram. An agent then works on it in its own workspace, a separate copy of the repository, so your folder stays as it is. " +
      "Only as many agents run at once as the project allows (\"Agents at once\", shown as 1/2 agents on an epic); other ready stories wait for a free slot. " +
      "Turn on \"Start stories automatically\" (or press Run epic) and an epic starts its ready stories by itself.",
  },
  {
    id: "inbox", title: "Answer what waits", show: "inbox", showLabel: "Open the inbox",
    alt: "The inbox with an agent's question and a story awaiting approval",
    text:
      "The Inbox gathers what needs a person: stories awaiting approval, questions of an agent, blocked stories, stories waiting for review, designs to approve and questions of the assistant. " +
      "Answer in the row, or open the story. " +
      "When an agent wants to run a command, Allow lets it once, Allow always remembers it for the project, and Deny refuses.",
  },
  {
    id: "review", title: "Review and accept", show: "review", showLabel: "Open a story in review",
    alt: "A story in review: the agent's hand-in, the review rounds, Accept and Return",
    text:
      "When an agent is done it hands in: what changed, each check as PASS, FAIL, NOT_RUN or BLOCKED, and what is left. " +
      "The test command runs, then a reviewer agent reads the work, in rounds up to the project's limit; each round is listed under Review. " +
      "You then press Accept, or Return with what to change, and the agent hands in again. " +
      "Accept merges into the main branch, or opens a pull request when the project is set to.",
  },
  {
    id: "accept-or-return", title: "Review: Accept or Return", show: "review", showLabel: "Open a story in review",
    alt: "A story in review: the Needs you list, then Accept and Return with a line under each",
    text:
      "Accept merges the story. Return sends your words to the agent, which fixes and hands in again. " +
      "Needs you lists what only you can judge, such as a look or a feeling; do those by hand first. " +
      "For example, if Needs you says to check the login on a phone, open it on a phone: if it looks right press Accept, and if the button is cut off press Return and write that the button is cut off on a phone.",
  },
  {
    id: "release", title: "Release a finished epic", show: "release", showLabel: "Open a finished epic",
    alt: "A finished epic's diagram: its version and the Release button at the end",
    text:
      "The roadmap is the epics, each with its stories and ending in a release. When all an epic's stories are done, a Release button appears at the end of its diagram. " +
      "Check the version (it becomes the tag, like v0.3.0) and press Release: Toknila checks, bumps the version and tags; in local mode you push main and the tag yourself, a GitHub project pushes as its steps say. " +
      "Nothing is tagged or pushed before you press it; a failed step stops the run and says which, and steps that passed are not done again.",
  },
  {
    id: "where", title: "Where things are", show: "tree", showLabel: "Show the project's tree",
    alt: "The sidebar: one tree per project, and Usage, Settings and Keyboard shortcuts at its foot",
    text:
      "The sidebar is one tree per project: Roadmap, Assistant (it plans epics on the roadmap, stories inside an epic), Inbox, Agents, Epics, Designs and Settings. " +
      "Right click a project and Hide from list takes it off the list while it keeps working: it waits under Show hidden (N), which says when something in it waits for you, and in the switcher (Cmd/Ctrl+K). " +
      "Its foot holds Usage (tokens and estimated cost by model, workspace, project, epic and story), Settings (Cmd/Ctrl+,) and Keyboard shortcuts (?). " +
      "A project's settings hold its test command, agents and review.",
  },
];
