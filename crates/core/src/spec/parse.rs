//! Reading a `tasks.md` into phases and tasks, keeping line numbers and byte offsets so the file
//! can be patched in place.

use super::{SpecError, SpecErrorKind};

/// A `tasks.md`: its title and every `## Phase` section, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecTasks {
    /// The `# Tasks: <title>` line, without the `Tasks:` prefix.
    pub title: Option<String>,
    pub phases: Vec<SpecPhase>,
}

/// One `## Phase N: ...` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecPhase {
    /// Line of the heading, 1-based.
    pub line: usize,
    /// The phase number as written, when it is a plain number.
    pub number: Option<u32>,
    /// Card title: the phase name; `US1: Title` for a user-story phase.
    pub title: String,
    /// The phase names a user story, so it runs in parallel with the other stories.
    pub story: bool,
    /// Text between the heading and the first task.
    pub prompt: String,
    pub tasks: Vec<SpecTask>,
}

/// One checkbox line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecTask {
    /// 1-based line.
    pub line: usize,
    /// Byte offset of the character inside the brackets.
    pub mark_offset: usize,
    /// Text after the checkbox, verbatim.
    pub text: String,
    pub done: bool,
}

/// Parses a `tasks.md`. A task line outside a phase or a malformed one is a typed error.
pub fn parse(text: &str) -> Result<SpecTasks, SpecError> {
    let mut spec = SpecTasks {
        title: None,
        phases: Vec::new(),
    };
    let mut in_phase = false;
    let mut fence: Option<String> = None;
    let mut prompt: Vec<&str> = Vec::new();
    let mut offset = 0;
    for (i, raw) in text.split_inclusive('\n').enumerate() {
        let line_no = i + 1;
        let line_start = offset;
        offset += raw.len();
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let head = line.trim_start_matches('\u{feff}');
        let trimmed = head.trim_start();
        if let Some(marker) = &fence {
            if trimmed.starts_with(marker.as_str()) {
                fence = None;
            }
            push_prompt(&spec, in_phase, &mut prompt, line);
            continue;
        }
        if let Some(marker) = fence_marker(trimmed) {
            fence = Some(marker);
            push_prompt(&spec, in_phase, &mut prompt, line);
            continue;
        }
        // Sub-headings (`### Tests for ...`) are part of the phase they sit in.
        if let Some(h) = heading(head).filter(|h| !matches!(h, Heading::Sub)) {
            finish_prompt(&mut spec, in_phase, &mut prompt);
            in_phase = false;
            match h {
                Heading::Title(t) if spec.title.is_none() && spec.phases.is_empty() => {
                    spec.title = Some(t);
                }
                Heading::Phase {
                    number,
                    title,
                    story,
                } => {
                    spec.phases.push(SpecPhase {
                        line: line_no,
                        number,
                        title,
                        story,
                        prompt: String::new(),
                        tasks: Vec::new(),
                    });
                    in_phase = true;
                }
                _ => {}
            }
            continue;
        }
        match task_line(trimmed) {
            Ok(Some((mark, done, body))) => {
                let Some(phase) = spec.phases.last_mut().filter(|_| in_phase) else {
                    return Err(SpecError::at(line_no, SpecErrorKind::TaskOutsidePhase));
                };
                if phase.tasks.is_empty() {
                    phase.prompt = join_prompt(&prompt);
                    prompt.clear();
                }
                phase.tasks.push(SpecTask {
                    line: line_no,
                    mark_offset: line_start + (line.len() - trimmed.len()) + mark,
                    text: body.to_owned(),
                    done,
                });
            }
            Ok(None) => push_prompt(&spec, in_phase, &mut prompt, line),
            Err(kind) => return Err(SpecError::at(line_no, kind)),
        }
    }
    finish_prompt(&mut spec, in_phase, &mut prompt);
    Ok(spec)
}

/// Prompt lines are only kept before the first task of the current phase.
fn push_prompt<'a>(spec: &SpecTasks, in_phase: bool, prompt: &mut Vec<&'a str>, line: &'a str) {
    if in_phase && spec.phases.last().is_some_and(|p| p.tasks.is_empty()) {
        prompt.push(line);
    }
}

/// A phase that ends without tasks keeps its text as the prompt.
fn finish_prompt(spec: &mut SpecTasks, in_phase: bool, prompt: &mut Vec<&str>) {
    if in_phase && let Some(p) = spec.phases.last_mut().filter(|p| p.tasks.is_empty()) {
        p.prompt = join_prompt(prompt);
    }
    prompt.clear();
}

fn join_prompt(lines: &[&str]) -> String {
    lines.join("\n").trim().to_owned()
}

fn fence_marker(trimmed: &str) -> Option<String> {
    let c = trimmed.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let n = trimmed.chars().take_while(|x| *x == c).count();
    (n >= 3).then(|| c.to_string().repeat(n))
}

enum Heading {
    Title(String),
    Phase {
        number: Option<u32>,
        title: String,
        story: bool,
    },
    /// A heading of level 3 or deeper.
    Sub,
}

/// A markdown heading line: `# Tasks: X` (title), `## Phase N: X` (phase) or any other.
fn heading(line: &str) -> Option<Heading> {
    let level = line.bytes().take_while(|b| *b == b'#').count();
    let rest = line[level..]
        .strip_prefix(' ')
        .filter(|_| (1..=6).contains(&level))?;
    let rest = rest.trim();
    Some(match level {
        1 => {
            let t = strip_prefix_ci(rest, "tasks:").unwrap_or(rest);
            Heading::Title(t.trim().to_owned())
        }
        // Any other `##` section (Preconditions, Increment A) is a phase if it holds tasks; the
        // sections without tasks are dropped by the importer.
        2 => phase_heading(rest).unwrap_or_else(|| Heading::Phase {
            number: None,
            title: rest.to_owned(),
            story: false,
        }),
        _ => Heading::Sub,
    })
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &s[prefix.len()..])
}

fn phase_heading(rest: &str) -> Option<Heading> {
    let after = strip_prefix_ci(rest, "phase")?;
    let after = after.strip_prefix(char::is_whitespace)?.trim_start();
    let end = after
        .find(|c: char| c.is_whitespace() || c == ':')
        .unwrap_or(after.len());
    let (num, name) = after.split_at(end);
    let name = trim_separators(name);
    let number = num.parse().ok();
    let (title, story) = match story_label(name) {
        Some((label, "")) => (label, true),
        Some((label, rest)) => (format!("{label}: {rest}"), true),
        None if name.is_empty() => (format!("Phase {num}"), false),
        None => (name.to_owned(), false),
    };
    Some(Heading::Phase {
        number,
        title,
        story,
    })
}

fn trim_separators(s: &str) -> &str {
    let sep = |c: char| c.is_whitespace() || matches!(c, ':' | '-' | '\u{2013}' | '\u{2014}');
    s.trim_start_matches(sep).trim_end()
}

/// `User Story 3 - Title` into (`US3`, `Title`).
fn story_label(name: &str) -> Option<(String, &str)> {
    let after = strip_prefix_ci(name, "user story")?;
    let after = after.strip_prefix(char::is_whitespace)?.trim_start();
    let end = after
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after.len());
    let (digits, rest) = after.split_at(end);
    if digits.is_empty() || rest.starts_with(|c: char| c.is_alphanumeric()) {
        return None;
    }
    Some((format!("US{digits}"), trim_separators(rest)))
}

/// `- [ ] text` into (offset of the mark inside the trimmed line, done, text). `Ok(None)` for
/// anything that is not a checkbox (a link such as `- [docs](x)` included).
fn task_line(trimmed: &str) -> Result<Option<(usize, bool, &str)>, SpecErrorKind> {
    let Some(rest) = trimmed
        .strip_prefix("- [")
        .or_else(|| trimmed.strip_prefix("* ["))
    else {
        return Ok(None);
    };
    let Some(close) = rest.find(']') else {
        return Ok(None);
    };
    let inside = &rest[..close];
    if inside.chars().count() > 1 {
        return Ok(None);
    }
    let after = &rest[close + 1..];
    if after.starts_with(['(', ':']) {
        return Ok(None);
    }
    let done = match inside {
        " " => false,
        "x" | "X" => true,
        _ => return Err(SpecErrorKind::MalformedTask),
    };
    let body = after.trim();
    if body.is_empty() || !after.starts_with(char::is_whitespace) {
        return Err(SpecErrorKind::MalformedTask);
    }
    Ok(Some((3, done, body)))
}
