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
            "Read the plan of the project. Without arguments it answers the overview: every chain (name, goal, version, autostart, phase, done/total), every card of an unfinished chain as one short record (key, title, status, what it waits for, design_first, design names, counts of subtasks and threads, whether it waits for the owner), a finished chain as one record without cards, `designs` (with the path of each file you may Read) and `pins` with their answers. With `chain` it answers that chain with every card in full; with `card`, that one card in full (prompt, acceptance, subtasks, threads with their messages, review results, pull request state).",
            json!({
                "chain": text("Open this chain in full: its name or id"),
                "card": text("Open this card in full: its key (DEMO-3), id or title"),
            }),
            &[],
        ),
        tool(
            "show_design",
            "Look at one design of the project, also one the owner added a moment ago. It answers the design's name, kind, media type, size and the path of its file as text and, for a png, jpg or webp, the picture itself as image content: you see it without reading any file. For a pdf it answers the metadata and path only. A Figma link whose image was not fetched has no file, and the answer says so in words. Name the design by its id or its name (see `designs` in show_plan).",
            json!({ "design": text("The design: its id or its name") }),
            &["design"],
        ),
        tool(
            "design_question",
            "Pin a question on a region of a design, when the design does not say something you need (a state, a label, a behaviour). The pin shows on the mockup, on the project page and on the card page, and in the owner's inbox as a question of the assistant; the owner answers it in place with a short text. An answered pin is added to its card's prompt under `From the design`, and `show_plan` lists every pin with its answer under `pins`. Ask one thing per pin, and give the `card` it is about so the answer reaches that card.",
            json!({
                "design": text("The design: its file name in the project's library (or its id)"),
                "region": { "type": "object",
                    "description": "The part of the image the question is about, in whole percent of its width and height",
                    "properties": {
                        "x": { "type": "integer", "minimum": 0, "maximum": 100 },
                        "y": { "type": "integer", "minimum": 0, "maximum": 100 },
                        "w": { "type": "integer", "minimum": 1, "maximum": 100 },
                        "h": { "type": "integer", "minimum": 1, "maximum": 100 },
                    }, "required": ["x", "y", "w", "h"] },
                "text": text("The question, at most 500 characters"),
                "card": { "type": "string", "description": "The card it is about (key, id or title)" },
            }),
            &["design", "region", "text"],
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
                "designs": designs_param("The project's designs the card is about; the first is the card's thumbnail"),
                "model": model_param("Leave out for routine work: the project's model runs it"),
            }),
            &["chain", "title"],
        ),
        tool(
            "edit_card",
            "Change a card that has not started: its title, prompt, acceptance (replaced as a whole), checklist items to add or to rename. On a card the person is discussing with you, a prompt or acceptance edit becomes a proposal shown as a diff that the person accepts or rejects; it lands only on accept.",
            json!({
                "card": card,
                "title": text("New title"),
                "prompt": text("New prompt"),
                "acceptance": list_of_text("The new acceptance list; it replaces the old one"),
                "design_first": { "type": "boolean", "description": "Whether the agent shows a design for approval before any code" },
                "designs": designs_param("The card's designs; the list replaces the old one, [] clears it, left out keeps it"),
                "model": model_param("The card's model; an empty text clears it, left out keeps it"),
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
            "Open a thread on a card to settle a design, a flow or an architecture with the owner before the card is built. Only the owner approves it. A card gets one thread of each kind: a second open_thread of the same kind is refused with the number of the existing thread; post a version on it with thread_version instead.",
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

/// The `model` argument: an alias or a full model name; anything else is refused.
fn model_param(description: &str) -> Value {
    let aliases = toknila_core::MODEL_ALIASES.join(", ");
    json!({
        "type": "string",
        "description": format!("{description}. One of {aliases}, or a full model name (claude-…)"),
    })
}

/// The `designs` argument: references to designs of the project's library.
fn designs_param(description: &str) -> Value {
    json!({
        "type": "array",
        "description": description,
        "items": { "type": "object", "properties": {
            "design": text("The design's file name in the project's library (or its id)"),
            "region": { "type": "object",
                "description": "The part of the image that matters, in whole percent of its width and height; left out: the whole image",
                "properties": {
                    "x": { "type": "integer", "minimum": 0, "maximum": 100 },
                    "y": { "type": "integer", "minimum": 0, "maximum": 100 },
                    "w": { "type": "integer", "minimum": 1, "maximum": 100 },
                    "h": { "type": "integer", "minimum": 1, "maximum": 100 },
                }, "required": ["x", "y", "w", "h"] },
        }, "required": ["design"] },
    })
}
