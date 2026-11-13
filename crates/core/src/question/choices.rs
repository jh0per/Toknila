//! The choices an agent may put under a question: two to four short sentences a person can act
//! on, the recommended one first. A chosen choice reaches the agent as its text.

/// The fewest choices a question with choices carries.
pub const MIN_CHOICES: usize = 2;

/// The most choices a question carries.
pub const MAX_CHOICES: usize = 4;

/// Why choices are refused, or why a pick cannot be made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChoiceError {
    #[error("a question takes two to four choices, not {0}")]
    Count(usize),
    #[error("a choice cannot be empty")]
    Empty,
    #[error("there is no choice {0}: the question has {1}")]
    Index(usize, usize),
}

/// The choices as stored: trimmed; none, or two to four non-empty ones.
pub fn check_choices(choices: Vec<String>) -> Result<Vec<String>, ChoiceError> {
    if choices.is_empty() {
        return Ok(choices);
    }
    if !(MIN_CHOICES..=MAX_CHOICES).contains(&choices.len()) {
        return Err(ChoiceError::Count(choices.len()));
    }
    let trimmed: Vec<String> = choices.iter().map(|c| c.trim().to_owned()).collect();
    if trimmed.iter().any(String::is_empty) {
        return Err(ChoiceError::Empty);
    }
    Ok(trimmed)
}

/// The text of the choice at the 0-based `index`, which is what the agent is told.
pub fn choice_text(choices: &[String], index: usize) -> Result<String, ChoiceError> {
    choices
        .get(index)
        .cloned()
        .ok_or(ChoiceError::Index(index, choices.len()))
}
