//! Pins of the assistant on a design: a question about a region of a mockup, answered by the
//! person in place. An answered pin's question and answer go into the card's prompt under
//! [`SECTION`]. Pure rules; the daemon keeps the pins.

use serde::{Deserialize, Serialize};

use super::Region;
use crate::{DesignId, PinId, ProjectId, TicketKey, Timestamp};

/// The heading of the part of a card's prompt that carries the answered pins.
pub const SECTION: &str = "From the design";

/// Most characters of a question or of an answer.
pub const MAX_TEXT: usize = 500;

/// A question of the assistant pinned on a region of a design.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignPin {
    pub id: PinId,
    pub project: ProjectId,
    pub design: DesignId,
    /// The design's name when the pin was made.
    pub design_name: String,
    /// The pin's number among the pins of its design, from 1; the marker shows it.
    pub number: u32,
    pub region: Region,
    /// The question.
    pub text: String,
    /// The card the question is about, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<TicketKey>,
    /// The person's answer; absent while the pin is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    pub created_at: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answered_at: Option<Timestamp>,
}

impl DesignPin {
    /// Whether the person has not answered yet.
    pub fn is_open(&self) -> bool {
        self.answer.is_none()
    }

    /// The region in words, as the prompt and the plan name it.
    pub fn place(&self) -> String {
        let r = &self.region;
        format!("x {}%, y {}%, {}% wide, {}% tall", r.x, r.y, r.w, r.h)
    }

    /// The pin as the planner reads it in the plan: `#2 checkout.png (region): "question"`,
    /// then `open` or the answer.
    pub fn plan_line(&self) -> String {
        let state = match &self.answer {
            Some(a) => format!("answered: \"{a}\""),
            None => "open".to_owned(),
        };
        format!(
            "#{} {} ({}): \"{}\" - {}",
            self.number,
            self.design_name,
            self.place(),
            self.text,
            state
        )
    }
}

/// A question or an answer: one trimmed text, not empty, at most [`MAX_TEXT`] characters.
pub fn clean_text(raw: &str, what: &str) -> Result<String, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(format!("The {what} is empty."));
    }
    if t.chars().count() > MAX_TEXT {
        return Err(format!("The {what} is longer than {MAX_TEXT} characters."));
    }
    Ok(t.to_owned())
}

fn entry(p: &DesignPin) -> Option<String> {
    let answer = p.answer.as_ref()?;
    let one_line = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(format!(
        "- {} #{} ({}): {} - {}",
        p.design_name,
        p.number,
        p.place(),
        one_line(&p.text),
        one_line(answer)
    ))
}

/// The prompt with its [`SECTION`] set to the answered pins in `pins` (open ones are left out):
/// an existing section is replaced in place, a missing one is appended, and no answered pin
/// removes it. The rest of the prompt is untouched.
pub fn with_section(prompt: &str, pins: &[&DesignPin]) -> String {
    let heading = format!("## {SECTION}");
    let lines: Vec<&str> = prompt.lines().collect();
    let start = lines.iter().position(|l| l.trim_end() == heading);
    let (before, after): (&[&str], &[&str]) = match start {
        Some(i) => {
            let end = lines[i + 1..]
                .iter()
                .position(|l| l.starts_with("## "))
                .map_or(lines.len(), |n| i + 1 + n);
            (&lines[..i], &lines[end..])
        }
        None => (&lines[..], &[]),
    };
    let entries: Vec<String> = pins.iter().filter_map(|p| entry(p)).collect();
    let mut out = before.join("\n").trim_end().to_owned();
    if !entries.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&heading);
        out.push('\n');
        out.push_str(&entries.join("\n"));
    }
    let tail = after.join("\n");
    if !tail.trim().is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(tail.trim_end());
    }
    out
}

#[cfg(test)]
mod tests;
