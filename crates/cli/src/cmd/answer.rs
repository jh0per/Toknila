//! `tok answer <KEY> <n | words>`: answer the question a story's agent asked.

use anyhow::{Result, bail};
use toknila_api_types::NewAnswer;
use toknila_client::Client;
use toknila_core::question::AnswerKind;

/// What the person typed: the number of a choice (counted from 1), or free words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Said {
    Number(u32),
    Words(String),
}

/// A lone number picks that choice; anything else is free text.
pub(super) fn parse(words: &[String]) -> Result<Said> {
    let text = words.join(" ");
    let text = text.trim();
    if text.is_empty() {
        bail!("say which choice (a number) or write your answer");
    }
    match text.parse::<u32>() {
        Ok(0) => bail!("choices are counted from 1"),
        Ok(n) => Ok(Said::Number(n)),
        Err(_) => Ok(Said::Words(text.to_owned())),
    }
}

/// The wire body for what was said: a number is the 0-based index of the choice.
pub(super) fn body(said: Said) -> NewAnswer {
    let (choice, text) = match said {
        Said::Number(n) => (Some(n - 1), None),
        Said::Words(t) => (None, Some(t)),
    };
    NewAnswer {
        role: None,
        answer: AnswerKind::Text,
        text,
        choice,
        always: false,
    }
}

pub(super) fn run(c: &Client, ticket: &str, words: &[String], json_out: bool) -> Result<()> {
    c.answer(ticket, &body(parse(words)?))?;
    if json_out {
        println!(
            "{}",
            serde_json::json!({ "ticket": ticket, "answered": true })
        );
    } else {
        println!("{ticket}: answered");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &str) -> Vec<String> {
        s.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn a_lone_number_is_a_choice_counted_from_one_and_anything_else_is_text() {
        assert_eq!(parse(&words("2")).expect("n"), Said::Number(2));
        assert_eq!(body(Said::Number(1)).choice, Some(0));
        assert_eq!(body(Said::Number(2)).text, None);
        assert_eq!(
            parse(&words("wait a day")).expect("w"),
            Said::Words("wait a day".into())
        );
        assert_eq!(body(Said::Words("x".into())).text.as_deref(), Some("x"));
        assert!(parse(&words("0")).is_err());
        assert!(parse(&[]).is_err());
    }
}
