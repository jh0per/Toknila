//! Design-token ratchet: every stylesheet except `tokens.css` takes its colours and font sizes
//! from tokens. A raw hex colour or a `font-size` in px counts as a raw value. Each stylesheet
//! has a count in [`BASELINE`] (absent means zero) that may only go down, like the size ratchet
//! in `guards.rs`. A screen card lowers the count of the stylesheet it moves to tokens.

use std::path::{Path, PathBuf};

/// The stylesheet that owns the raw values; it is not counted.
const TOKENS: &str = "tokens.css";

/// The web sources the guard reads, relative to the repository root.
const STYLE_DIR: &str = "apps/web/src";

/// Raw values (hex colours plus px font sizes) per stylesheet when the guard was introduced.
/// Each may only shrink; the guard says when an entry can be lowered or removed.
const BASELINE: &[(&str, usize)] = &[("inbox.css", 2), ("usage.css", 1)];

/// Raw values in one stylesheet's text: hex colours, and `font-size` or `font` shorthand in px.
pub(crate) fn raw_values(css: &str) -> usize {
    let text = strip_comments(css);
    text.split([';', '{', '}']).map(chunk_raw_values).sum()
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = match rest[start..].find("*/") {
            Some(end) => &rest[start + end + 2..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// One declaration (or selector) chunk: its hex colours, and whether it sets a px font size.
fn chunk_raw_values(chunk: &str) -> usize {
    let chunk = chunk.trim();
    let mut n = hex_colours(chunk);
    let size_decl = chunk
        .strip_prefix("font-size")
        .or_else(|| chunk.strip_prefix("font:"))
        .is_some_and(has_px);
    if size_decl {
        n += 1;
    }
    n
}

/// A number directly followed by `px`, as in `12px` or `0.5px`.
fn has_px(value: &str) -> bool {
    let b = value.as_bytes();
    (2..b.len()).any(|i| b[i] == b'x' && b[i - 1] == b'p' && (b[i - 2].is_ascii_digit()))
}

/// `#` followed by 3, 4, 6 or 8 hex digits and then a non-word character.
fn hex_colours(chunk: &str) -> usize {
    let b = chunk.as_bytes();
    let mut n = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'#' {
            let digits = b[i + 1..]
                .iter()
                .take_while(|c| c.is_ascii_hexdigit())
                .count();
            let after = b.get(i + 1 + digits).copied();
            let word_after = after.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_');
            if matches!(digits, 3 | 4 | 6 | 8) && !word_after {
                n += 1;
            }
            i += 1 + digits;
        } else {
            i += 1;
        }
    }
    n
}

/// Everything wrong with the counted stylesheets against the baseline.
pub(crate) fn problems(measured: &[(String, usize)], baseline: &[(&str, usize)]) -> Vec<String> {
    let mut out = Vec::new();
    for (path, n) in measured {
        let cap = baseline
            .iter()
            .find(|(p, _)| p == path)
            .map_or(0, |(_, cap)| *cap);
        if *n > cap {
            out.push(format!(
                "{path}: {n} raw hex colours or px font sizes, allowed {cap}; use a token from tokens.css"
            ));
        } else if *n < cap {
            out.push(format!(
                "{path}: down to {n} raw values; lower its BASELINE entry from {cap}"
            ));
        }
    }
    for (path, _) in baseline {
        if !measured.iter().any(|(p, _)| p == path) {
            out.push(format!(
                "{path}: listed in BASELINE but gone; remove the entry"
            ));
        }
    }
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if path.is_dir() {
            if !matches!(name, "node_modules" | "dist" | "gen") {
                collect(&path, out)?;
            }
        } else if name.ends_with(".css") && name != TOKENS {
            out.push(path);
        }
    }
    Ok(())
}

/// Runs the guard on the repository at `root`.
pub fn run(root: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    collect(&root.join(STYLE_DIR), &mut files)?;
    files.sort();
    let mut measured = Vec::with_capacity(files.len());
    for path in &files {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let rel = path
            .strip_prefix(root.join(STYLE_DIR))
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        measured.push((rel, raw_values(&text)));
    }
    // Control: the walk must have found the shared stylesheets, or it read nothing.
    if !measured.iter().any(|(p, _)| p == "style.css") {
        return Err("css guard did not find style.css; probe is blind".into());
    }
    let found = problems(&measured, BASELINE);
    if !found.is_empty() {
        return Err(format!("design token guard:\n  {}", found.join("\n  ")));
    }
    eprintln!(
        "guard css-tokens (no raw hex or px font-size outside {TOKENS}, {} files): PASS",
        measured.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_hex_colours_and_px_font_sizes() {
        let css = ".a { color: #fff; background: #1a2b3c; font-size: 12px; }";
        assert_eq!(raw_values(css), 3);
    }

    #[test]
    fn tokens_comments_and_ids_are_not_raw_values() {
        let css = "/* #fff 12px */ .a { color: var(--fg); font-size: var(--fs-m); }\n#root { height: 100%; }\n.b { font-size: 1.2em; margin: 12px; }";
        assert_eq!(raw_values(css), 0);
    }

    #[test]
    fn font_shorthand_with_a_px_size_counts() {
        assert_eq!(raw_values(".a { font: 12px/1.4 sans-serif; }"), 1);
        assert_eq!(raw_values(".a { font: var(--fs-m)/1.4 var(--ui); }"), 0);
    }

    #[test]
    fn a_new_raw_value_fails_and_the_ratchet_tightens() {
        let rows = vec![("a.css".to_owned(), 2)];
        assert_eq!(problems(&rows, &[("a.css", 2)]), Vec::<String>::new());
        assert_eq!(problems(&rows, &[]).len(), 1);
        assert!(problems(&rows, &[("a.css", 3)])[0].contains("lower its BASELINE"));
        assert!(problems(&[], &[("a.css", 1)])[0].contains("gone"));
    }
}
