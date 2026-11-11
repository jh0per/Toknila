//! The schema of the planner's tools, as `tools/list` answers it.

use serde_json::{Value, json};

use super::super::mcp_tools::{list_of_text, text, tool};

/// What `tools/list` answers for a planner.
pub(in crate::cmd) fn list() -> Vec<Value> {
    let chain = text("The chain: its name or id");
    let card = text("The card: its key (DEMO-3), id or title");
    let thread = text("The thread's number on the card (1, 2, ...), or its id");
    vec![
        tool(
            "show_plan",
            "Read the whole plan of the project: every chain with its cards in dependency order (the order is the priority), each card's status, prompt, acceptance, subtasks, what it waits for, threads, review results and pull request state.",
            json!({}),
            &[],
        ),
        tool(
            "new_chain",
            "Create a chain. Its cards wait for the owner to start them: a chain you create never starts cards by itself.",
            json!({ "name": text("Short unique name"), "goal": text("What the chain is for") }),
            &["name"],
        ),
        tool(
            "add_card",
            "Add a card to a chain, at its end or after the cards named in `after`. Refused in a chain that starts its cards by itself.",
            json!({
                "chain": chain,
                "title": text("Short title"),
                "prompt": text("The task, written so that an agent who sees only this card and the repository can do it"),
                "acceptance": list_of_text("Testable acceptance items: a command to run, a behaviour to observe"),
                "after": list_of_text("Cards (keys) the new card waits for; empty: the end of the chain"),
                "subtasks": list_of_text("Checklist items"),
                "design_first": { "type": "boolean", "description": "True for a card that changes what the person sees (screens, flows): its agent then shows a design for approval before any code" },
            }),
            &["chain", "title"],
        ),
        tool(
            "edit_card",
            "Change a card that has not started: its title, prompt, acceptance (replaced as a whole), checklist items to add or to rename.",
            json!({
                "card": card,
                "title": text("New title"),
                "prompt": text("New prompt"),
                "acceptance": list_of_text("The new acceptance list; it replaces the old one"),
                "design_first": { "type": "boolean", "description": "Whether the agent shows a design for approval before any code" },
                "add_subtasks": list_of_text("Checklist items to append"),
                "edit_subtasks": { "type": "array", "description": "Checklist items to rename",
                    "items": { "type": "object", "properties": {
                        "position": { "type": "integer", "minimum": 1 },
                        "text": text("The new text"),
                    }, "required": ["position", "text"] } },
            }),
            &["card"],
        ),
        tool(
            "show_card",
            "Open a card in the owner's window (its page, with the designs on it), when they have this project's assistant or chain open. They get a small notice with a Back button. Use it to point at a card you are talking about; it changes nothing in the plan.",
            json!({ "card": card }),
            &["card"],
        ),
        tool(
            "suggest",
            "Say the owner's next step. It shows at the top of the assistant's screen and as the first row of the inbox, with one button for its action; it replaces your previous suggestion, and it ends by itself when the action is done or the card moves on. `action` is one of start, answer, accept, release and goes with the `card` it is about (for release, any card of the chain). With neither, it says that nothing waits for the owner. You never do the action: the owner clicks.",
            json!({
                "text": text("The step in a line or two"),
                "card": { "type": "string", "description": "The card the step is about (id, key or title)" },
                "action": { "type": "string", "enum": ["start", "answer", "accept", "release"],
                    "description": "What the button does; needs `card`" },
            }),
            &["text"],
        ),
        tool(
            "add_dependency",
            "Make `to` wait for `from` in a chain. Refused in a chain that starts its cards by itself.",
            json!({ "chain": chain, "from": text("Card key"), "to": text("Card key") }),
            &["chain", "from", "to"],
        ),
        tool(
            "remove_dependency",
            "Remove the wait of `to` for `from`. Refused in a chain that starts its cards by itself.",
            json!({ "chain": chain, "from": text("Card key"), "to": text("Card key") }),
            &["chain", "from", "to"],
        ),
        tool(
            "move_card",
            "Put a card directly before or after another card of its chain; the dependencies around it are rewired. A card that has started cannot move.",
            json!({
                "chain": chain,
                "card": card,
                "anchor": text("The card it goes next to"),
                "place": { "type": "string", "enum": ["before", "after"] },
            }),
            &["chain", "card", "anchor", "place"],
        ),
        tool(
            "open_thread",
            "Open a thread on a card to settle a design, a flow or an architecture with the owner before the card is built. Only the owner approves it.",
            json!({
                "card": card,
                "kind": { "type": "string", "enum": ["design", "flow", "architecture", "other"] },
                "title": text("What the thread is about"),
            }),
            &["card", "kind", "title"],
        ),
        tool(
            "thread_message",
            "Post a message in a thread of a card.",
            json!({ "card": card, "thread": thread, "text": text("The message") }),
            &["card", "thread", "text"],
        ),
        tool(
            "thread_version",
            "Post the next version of a thread's artifact (a diagram, a screen description). A new version reopens an approved thread.",
            json!({
                "card": card,
                "thread": thread,
                "format": { "type": "string", "enum": ["mermaid", "markdown", "html"] },
                "body": text("The artifact, at most 512 KiB"),
            }),
            &["card", "thread", "format", "body"],
        ),
    ]
}
