//! The comment template: one short shape for people and agents alike.
//!
//! ```markdown
//! # [decision] rusqlite and one writer
//!
//! **What:** use rusqlite behind a single writer task
//!
//! **Why:** transitions run strictly in order
//! ```
//!
//! [`render`] writes it, [`parse`] reads it back. Empty fields are omitted. Labels are written in
//! English; the Ukrainian labels from the project brief (`інструкція`, `**Що:**`, ...) are
//! accepted on parse so hand-written comments work in either language.

use crate::wire::wire_name;
use crate::{CommentBody, CommentKind};

/// Why a markdown comment could not be read. `line` is 1-based.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TemplateError {
    #[error("line {line}: expected a heading like `# [instruction] <gist>`")]
    MissingHeading { line: usize },
    #[error("line {line}: unknown comment kind {kind:?} (instruction, report or decision)")]
    UnknownKind { line: usize, kind: String },
    #[error("line {line}: the heading has no gist after the kind")]
    EmptyGist { line: usize },
    #[error("line {line}: text outside a field; start it with a label such as `**What:**`")]
    StrayText { line: usize },
    #[error("line {line}: field {label:?} appears twice")]
    DuplicateField { line: usize, label: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    What,
    Why,
    HowToVerify,
    Alternatives,
    Consequences,
}

impl Field {
    const ALL: [Field; 5] = [
        Field::What,
        Field::Why,
        Field::HowToVerify,
        Field::Alternatives,
        Field::Consequences,
    ];

    fn label(self) -> &'static str {
        match self {
            Field::What => "What",
            Field::Why => "Why",
            Field::HowToVerify => "How to verify",
            Field::Alternatives => "Alternatives",
            Field::Consequences => "Consequences",
        }
    }

    fn from_label(label: &str) -> Option<Field> {
        let label = label.trim().to_lowercase();
        let field = match label.as_str() {
            "what" | "що" => Field::What,
            "why" | "чому" => Field::Why,
            "how to verify" | "як перевірити" => Field::HowToVerify,
            "alternatives" | "альтернативи" => Field::Alternatives,
            "consequences" | "consequences / risks" | "наслідки" | "наслідки / ризики" => {
                Field::Consequences
            }
            _ => return None,
        };
        Some(field)
    }

    fn get(self, body: &CommentBody) -> Option<&String> {
        match self {
            Field::What => body.what.as_ref(),
            Field::Why => body.why.as_ref(),
            Field::HowToVerify => body.how_to_verify.as_ref(),
            Field::Alternatives => body.alternatives.as_ref(),
            Field::Consequences => body.consequences.as_ref(),
        }
    }

    fn slot(self, body: &mut CommentBody) -> &mut Option<String> {
        match self {
            Field::What => &mut body.what,
            Field::Why => &mut body.why,
            Field::HowToVerify => &mut body.how_to_verify,
            Field::Alternatives => &mut body.alternatives,
            Field::Consequences => &mut body.consequences,
        }
    }
}

fn kind_from_word(word: &str) -> Option<CommentKind> {
    match word.trim().to_lowercase().as_str() {
        "instruction" | "інструкція" => Some(CommentKind::Instruction),
        "report" | "звіт" => Some(CommentKind::Report),
        "decision" | "рішення" => Some(CommentKind::Decision),
        _ => None,
    }
}

/// The canonical form of a body: gist on one line, fields trimmed, blank fields dropped.
/// `parse(render(b))` returns `normalize(b)`.
pub fn normalize(body: &CommentBody) -> CommentBody {
    let gist = body.gist.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = CommentBody {
        gist,
        ..CommentBody::default()
    };
    for field in Field::ALL {
        let value = field
            .get(body)
            .map(|v| clean_value(v))
            .filter(|v| !v.is_empty());
        *field.slot(&mut out) = value;
    }
    out
}

/// Trims the value and each line's trailing whitespace; keeps inner line breaks.
fn clean_value(value: &str) -> String {
    let lines: Vec<&str> = value.lines().map(str::trim_end).collect();
    lines.join("\n").trim().to_owned()
}

/// Renders a comment as markdown in the template shape.
pub fn render(kind: CommentKind, body: &CommentBody) -> String {
    let body = normalize(body);
    let mut out = format!(
        "# [{}] {}\n",
        wire_name(&kind).unwrap_or_default(),
        body.gist
    );
    for field in Field::ALL {
        if let Some(value) = field.get(&body) {
            out.push('\n');
            out.push_str(&format!("**{}:**", field.label()));
            for (i, line) in value.lines().enumerate() {
                out.push_str(if i == 0 { " " } else { "\n" });
                out.push_str(&escape_line(line));
            }
            out.push('\n');
        }
    }
    out
}

/// A value line that could be mistaken for a label (or that starts with the escape character)
/// gets a leading backslash; [`unescape_line`] strips exactly one.
fn escape_line(line: &str) -> String {
    if line.starts_with('\\') || line.starts_with("**") || line.starts_with("# ") {
        format!("\\{line}")
    } else {
        line.to_owned()
    }
}

fn unescape_line(line: &str) -> &str {
    line.strip_prefix('\\').unwrap_or(line)
}

/// `**Label:** rest` → (field, rest).
fn field_line(line: &str) -> Option<(Field, &str)> {
    let rest = line.strip_prefix("**")?;
    let (label, rest) = rest.split_once(":**")?;
    let field = Field::from_label(label)?;
    Some((field, rest.strip_prefix(' ').unwrap_or(rest)))
}

/// Reads a markdown comment back into its kind and body (normalized).
pub fn parse(markdown: &str) -> Result<(CommentKind, CommentBody), TemplateError> {
    let mut lines = markdown.lines().enumerate().map(|(i, l)| (i + 1, l));

    let (line_no, heading) = loop {
        match lines.next() {
            Some((_, l)) if l.trim().is_empty() => continue,
            Some((n, l)) => break (n, l.trim()),
            None => return Err(TemplateError::MissingHeading { line: 1 }),
        }
    };
    let inner = heading
        .strip_prefix("# [")
        .ok_or(TemplateError::MissingHeading { line: line_no })?;
    let (word, gist) = inner
        .split_once(']')
        .ok_or(TemplateError::MissingHeading { line: line_no })?;
    // The brief's template writes alternatives in the heading: `[інструкція | звіт | рішення]`.
    let kind = kind_from_word(word).ok_or_else(|| TemplateError::UnknownKind {
        line: line_no,
        kind: word.to_owned(),
    })?;
    let gist = gist.trim();
    if gist.is_empty() {
        return Err(TemplateError::EmptyGist { line: line_no });
    }

    let mut body = CommentBody {
        gist: gist.to_owned(),
        ..CommentBody::default()
    };
    let mut current: Option<(Field, Vec<String>)> = None;
    let mut seen: Vec<Field> = Vec::new();

    let flush = |current: &mut Option<(Field, Vec<String>)>, body: &mut CommentBody| {
        if let Some((field, value)) = current.take() {
            *field.slot(body) = Some(value.join("\n"));
        }
    };

    for (n, line) in lines {
        if let Some((field, rest)) = field_line(line) {
            if seen.contains(&field) {
                return Err(TemplateError::DuplicateField {
                    line: n,
                    label: field.label().to_owned(),
                });
            }
            seen.push(field);
            flush(&mut current, &mut body);
            current = Some((field, vec![unescape_line(rest).to_owned()]));
        } else if let Some((_, value)) = current.as_mut() {
            value.push(unescape_line(line).to_owned());
        } else if !line.trim().is_empty() {
            return Err(TemplateError::StrayText { line: n });
        }
    }
    flush(&mut current, &mut body);
    Ok((kind, normalize(&body)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn full() -> CommentBody {
        CommentBody {
            gist: "rusqlite and one writer".to_owned(),
            what: Some("Use rusqlite behind one writer task".to_owned()),
            why: Some("Transitions run strictly in order.\nNo races.".to_owned()),
            how_to_verify: Some("cargo test -p toknila-daemon".to_owned()),
            alternatives: Some("sqlx: needs DATABASE_URL at build time".to_owned()),
            consequences: Some("All writes go through one queue".to_owned()),
        }
    }

    #[test]
    fn renders_template_shape_and_omits_empty_fields() {
        let body = CommentBody {
            gist: "Solid".to_owned(),
            what: Some("Use Solid".to_owned()),
            why: Some("   ".to_owned()),
            ..CommentBody::default()
        };
        assert_eq!(
            render(CommentKind::Instruction, &body),
            "# [instruction] Solid\n\n**What:** Use Solid\n"
        );
    }

    #[test]
    fn markdown_structure_markdown_is_identical() {
        let md = render(CommentKind::Decision, &full());
        let (kind, body) = parse(&md).expect("parse");
        assert_eq!(kind, CommentKind::Decision);
        assert_eq!(body, full());
        assert_eq!(render(kind, &body), md);
    }

    #[test]
    fn parses_the_ukrainian_template_from_the_brief() {
        let md = "# [рішення] один писар\n\n**Що:** rusqlite\n**Чому:** без гонок\n\
                  **Як перевірити:** cargo test\n**Альтернативи:** sqlx\n\
                  **Наслідки / ризики:** одна черга\n";
        let (kind, body) = parse(md).expect("parse");
        assert_eq!(kind, CommentKind::Decision);
        assert_eq!(body.what.as_deref(), Some("rusqlite"));
        assert_eq!(body.why.as_deref(), Some("без гонок"));
        assert_eq!(body.how_to_verify.as_deref(), Some("cargo test"));
        assert_eq!(body.alternatives.as_deref(), Some("sqlx"));
        assert_eq!(body.consequences.as_deref(), Some("одна черга"));
    }

    #[test]
    fn value_lines_that_look_like_labels_survive() {
        let body = CommentBody {
            gist: "tricky".to_owned(),
            what: Some("first\n**Why:** not a label\n\\backslash\n# not a heading".to_owned()),
            ..CommentBody::default()
        };
        let (_, back) = parse(&render(CommentKind::Report, &body)).expect("parse");
        assert_eq!(back, body);
    }

    #[test]
    fn errors_carry_line_numbers() {
        assert_eq!(parse(""), Err(TemplateError::MissingHeading { line: 1 }));
        assert_eq!(
            parse("\n\nhello"),
            Err(TemplateError::MissingHeading { line: 3 })
        );
        assert!(matches!(
            parse("# [idea] x"),
            Err(TemplateError::UnknownKind { line: 1, .. })
        ));
        assert_eq!(
            parse("# [report]   "),
            Err(TemplateError::EmptyGist { line: 1 })
        );
        assert_eq!(
            parse("# [report] x\n\nloose"),
            Err(TemplateError::StrayText { line: 3 })
        );
        assert!(matches!(
            parse("# [report] x\n**What:** a\n**Що:** b"),
            Err(TemplateError::DuplicateField { line: 3, .. })
        ));
    }

    fn opt_text() -> impl Strategy<Value = Option<String>> {
        proptest::option::of("[ -~\n\\\\*#a-zA-Zа-яіїє]{0,60}")
    }

    proptest! {
        #[test]
        fn structure_markdown_structure_roundtrips(
            gist in "[ -~а-яіїє]{1,40}",
            what in opt_text(), why in opt_text(), how in opt_text(),
            alt in opt_text(), cons in opt_text(),
            kind in prop_oneof![
                Just(CommentKind::Instruction), Just(CommentKind::Report), Just(CommentKind::Decision)
            ],
        ) {
            let body = CommentBody {
                gist, what, why, how_to_verify: how, alternatives: alt, consequences: cons,
            };
            prop_assume!(!normalize(&body).gist.is_empty());
            let md = render(kind, &body);
            let (k, back) = parse(&md).map_err(|e| TestCaseError::fail(format!("{e}\n{md}")))?;
            prop_assert_eq!(k, kind);
            prop_assert_eq!(&back, &normalize(&body));
            prop_assert_eq!(render(k, &back), md);
        }
    }
}
