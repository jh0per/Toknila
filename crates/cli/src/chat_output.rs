//! `tok planner chat`: the project's conversation with its assistant as text, oldest first.

use chrono::{DateTime, FixedOffset};
use toknila_core::conversation::{About, Author, Message};

/// When a message was said, in `offset`; `--------------` if the timestamp is out of range.
fn when(m: &Message, offset: FixedOffset) -> String {
    DateTime::from_timestamp_millis(m.at.0)
        .map(|t| {
            t.with_timezone(&offset)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "----------------".to_owned())
}

/// The card or chain a message was about, as the chat's chip says it.
fn about(m: &Message) -> String {
    match &m.about {
        Some(About::Card { key }) => format!(" [{key}]"),
        Some(About::Chain { name }) => format!(" [epic {name}]"),
        Some(About::Roadmap) => " [roadmap]".to_owned(),
        None => String::new(),
    }
}

/// The lines of one message: a head with the time, the author and the context, then the text
/// (a line of Toknila's is one line with a dot). Further lines of a text are indented.
pub fn message_lines(m: &Message, offset: FixedOffset) -> Vec<String> {
    let time = when(m, offset);
    if m.author == Author::Toknila {
        return vec![format!("{time}  · {}{}", m.text, about(m))];
    }
    let who = if m.author == Author::Person {
        "You"
    } else {
        "Assistant"
    };
    let mut lines = m.text.lines();
    let first = lines.next().unwrap_or_default();
    let mut out = vec![format!("{time}  {who}{}: {first}", about(m))];
    out.extend(lines.map(|l| format!("                  {l}")));
    if !m.attachments.is_empty() {
        out.push(format!(
            "                  (attached: {})",
            m.attachments.join(", ")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use toknila_core::Timestamp;

    fn msg(author: Author, text: &str, about: Option<About>) -> Message {
        Message {
            id: "01JBQ7ZJ0000000000000000M1".into(),
            author,
            text: text.into(),
            about,
            attachments: vec![],
            thread: None,
            at: Timestamp(1_700_000_000_000),
        }
    }

    #[test]
    fn the_record_reads_as_who_said_what_with_toknilas_lines_as_dots() {
        let utc = FixedOffset::east_opt(0).expect("offset");
        let card = || {
            Some(About::Card {
                key: "DEMO-2".parse().expect("key"),
            })
        };
        assert_eq!(
            message_lines(&msg(Author::Person, "Add a story\nafter DEMO-1", None), utc),
            [
                "2023-11-14 22:13  You: Add a story",
                "                  after DEMO-1"
            ]
        );
        assert_eq!(
            message_lines(&msg(Author::Assistant, "Done", card()), utc),
            ["2023-11-14 22:13  Assistant [DEMO-2]: Done"]
        );
        assert_eq!(
            message_lines(&msg(Author::Toknila, "added story DEMO-2", card()), utc),
            ["2023-11-14 22:13  · added story DEMO-2 [DEMO-2]"]
        );
    }
}
