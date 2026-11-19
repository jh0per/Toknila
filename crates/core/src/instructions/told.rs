//! What the person told this story's agent, as the reviewer reads it: every instruction a person
//! wrote on the card and every question of the agent with the person's answer, in the order they
//! happened. It is part of the task: where it differs from the story's text, the later word wins.

use super::RunBrief;
use crate::{Actor, Comment, CommentKind, Timestamp, template};

/// The size, in characters of the rendered entries, over which the oldest entries are left out.
/// The newest entry is always shown, whole, however long it is.
pub const TOLD_MAX_CHARS: usize = 6000;

/// One thing a person told the story's agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefTold {
    /// When the person wrote it.
    pub at: Timestamp,
    /// The agent's question this answers (its first line and the rest, as it asked); `None` for an
    /// instruction the person gave unasked.
    pub question: Option<String>,
    /// The person's words, whole: typed words as typed, a pressed choice as the choice's own text.
    pub text: String,
}

impl BriefTold {
    /// The person's words in the card's comments, oldest first. Only a comment a person wrote as
    /// an instruction counts; an agent's report or decision, the system's and the assistant's
    /// comments never do. An agent's plain report (a gist and at most a "what", the shape of an
    /// `ask_human` question) that comes right before a person's instruction is shown as the
    /// question it answers.
    pub fn of(comments: &[Comment]) -> Vec<Self> {
        let mut sorted: Vec<&Comment> = comments.iter().collect();
        sorted.sort_by_key(|c| c.created_at);
        let mut out = Vec::new();
        let mut asked: Option<&Comment> = None;
        for c in sorted {
            match (c.author, c.kind) {
                (Actor::Human, CommentKind::Instruction) => {
                    out.push(Self {
                        at: c.created_at,
                        question: asked.take().map(|q| plain(&q.body)),
                        text: plain(&c.body),
                    });
                }
                (Actor::Agent, CommentKind::Report) if is_plain(&c.body) => asked = Some(c),
                // Anything else of the agent or the system lies between: no longer a question
                // that the next instruction answers.
                (Actor::Human, _) | (Actor::Assistant, _) => {}
                _ => asked = None,
            }
        }
        out
    }

    fn render(&self) -> String {
        let mut out = String::new();
        if let Some(q) = &self.question {
            out.push_str("- The agent asked:\n");
            quote(&mut out, q);
            out.push_str(&format!(
                "  The person answered ({}):\n",
                self.at.utc_text()
            ));
        } else {
            out.push_str(&format!(
                "- The person instructed the agent ({}):\n",
                self.at.utc_text()
            ));
        }
        quote(&mut out, &self.text);
        out
    }
}

fn quote(out: &mut String, text: &str) {
    for line in text.lines() {
        out.push_str(&format!("  > {line}\n").replace("  > \n", "  >\n"));
    }
}

/// A report with no template fields beyond the gist and the "what": how a question is stored.
fn is_plain(b: &crate::CommentBody) -> bool {
    b.why.is_none()
        && b.how_to_verify.is_none()
        && b.alternatives.is_none()
        && b.consequences.is_none()
        && b.person.is_empty()
}

/// A comment's words as written: the gist, then the rest, with no template heading.
fn plain(b: &crate::CommentBody) -> String {
    let full = template::render(CommentKind::Instruction, b);
    let rest = full.split_once('\n').map_or("", |(_, r)| r).trim();
    if rest.is_empty() {
        b.gist.trim().to_owned()
    } else {
        format!("{}\n{rest}", b.gist.trim())
    }
}

/// The section "What the person told this story's agent"; says so when nothing was told.
pub(super) fn section(brief: &RunBrief<'_>) -> String {
    let mut out = String::from(
        "\n## What the person told this story's agent\n\n\
         This is part of the task. Every instruction a person wrote on the card and every \
         question of the agent with the person's answer, oldest first, whole, with its date \
         (UTC). Where it differs from the task above, the person's later word wins: judge each \
         item against the task as changed and name the instruction that changed it in the \
         verdict. Only a person's words are here; the agent's reports are not.\n\n",
    );
    if brief.told.is_empty() {
        out.push_str("The person told the agent nothing on this story.\n");
        return out;
    }
    let rendered: Vec<String> = brief.told.iter().map(BriefTold::render).collect();
    let mut keep = rendered.len();
    let mut size = 0;
    // From the newest back: the newest entry is always kept, then older ones while they fit.
    for (i, r) in rendered.iter().enumerate().rev() {
        size += r.chars().count();
        if size > TOLD_MAX_CHARS && i + 1 < rendered.len() {
            break;
        }
        keep = i;
    }
    if keep > 0 {
        out.push_str(&format!(
            "{keep} earlier {} left out: the oldest, to keep the record short (over {TOLD_MAX_CHARS} characters).\n",
            if keep == 1 { "entry is" } else { "entries are" }
        ));
    }
    for r in &rendered[keep..] {
        out.push_str(r);
    }
    out
}
