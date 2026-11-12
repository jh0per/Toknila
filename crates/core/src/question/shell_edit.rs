//! Shell edits are refused, not asked about. A Bash call that writes a file through a shell (a
//! heredoc redirected into a file, `sed -i`, `perl -pi`, a `python - <<` script, `tee` into a path)
//! costs the person a prompt although the Edit and Write tools are approved without asking. The
//! permission hook denies such a call at once with the reason the instructions give. Reads
//! (`cat`, `sed -n`, `grep`) and writes into the agent's own scratchpad are left alone.
//!
//! Pure: it reads the text of the tool call (`Bash: <command>`) and judges it; the daemon says
//! which folders count as scratch and delivers the denial.

use super::Decision;

/// What the agent is told when its shell edit is refused.
pub const SHELL_EDIT_DENIAL: &str = "Shell edits are refused here: change files with the Edit tool (existing files) and the Write tool (new files). They are approved without asking, while a python, sed, perl, tee or heredoc edit is not. Redo this change with Edit or Write; reads (cat, sed -n, grep) and writes into your scratchpad are fine in the shell.";

/// The refusal for `tool` (the text of a tool call, `Bash: <command>`) when it edits a file
/// through a shell, else `None`. `scratch` are extra folders (besides any `scratchpad` folder and
/// `$TMPDIR`) whose files the agent may write freely.
pub fn shell_edit_denial(tool: &str, scratch: &[String]) -> Option<Decision> {
    let command = tool.trim().strip_prefix("Bash:")?.trim();
    edits_a_file(command, scratch, 0).then(|| Decision::Deny {
        message: SHELL_EDIT_DENIAL.to_owned(),
    })
}

enum Tok {
    Word(String),
    Op(String),
}

/// Splits shell text into words and operators. Quotes are removed, heredoc bodies are skipped, a
/// newline is a `;`. Not a shell parser: enough to see what a command writes.
fn tokenize(src: &str) -> Vec<Tok> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut word = String::new();
    let mut has_word = false;
    let mut pending: Vec<String> = Vec::new();
    let mut want_delim = false;
    let mut i = 0;
    let flush = |out: &mut Vec<Tok>,
                 word: &mut String,
                 has_word: &mut bool,
                 want: &mut bool,
                 pending: &mut Vec<String>| {
        if *has_word {
            if *want {
                pending.push(word.clone());
                *want = false;
            }
            out.push(Tok::Word(std::mem::take(word)));
            *has_word = false;
        }
    };
    while i < c.len() {
        let ch = c[i];
        match ch {
            '\'' => {
                has_word = true;
                i += 1;
                while i < c.len() && c[i] != '\'' {
                    word.push(c[i]);
                    i += 1;
                }
                i += 1;
            }
            '"' => {
                has_word = true;
                i += 1;
                while i < c.len() && c[i] != '"' {
                    if c[i] == '\\' && i + 1 < c.len() {
                        i += 1;
                    }
                    word.push(c[i]);
                    i += 1;
                }
                i += 1;
            }
            '\\' => {
                if i + 1 < c.len() {
                    word.push(c[i + 1]);
                    has_word = true;
                }
                i += 2;
            }
            ' ' | '\t' | '\r' => {
                flush(
                    &mut out,
                    &mut word,
                    &mut has_word,
                    &mut want_delim,
                    &mut pending,
                );
                i += 1;
            }
            '\n' | ';' => {
                flush(
                    &mut out,
                    &mut word,
                    &mut has_word,
                    &mut want_delim,
                    &mut pending,
                );
                out.push(Tok::Op(";".to_owned()));
                i += 1;
                if ch == '\n' {
                    for delim in std::mem::take(&mut pending) {
                        // The body runs to a line that is the delimiter, or to the end.
                        while i < c.len() {
                            let end = c[i..]
                                .iter()
                                .position(|x| *x == '\n')
                                .map_or(c.len(), |p| i + p);
                            let line: String = c[i..end].iter().collect();
                            i = (end + 1).min(c.len());
                            if line.trim() == delim {
                                break;
                            }
                        }
                    }
                }
            }
            '|' | '&' if c.get(i + 1) == Some(&ch) => {
                flush(
                    &mut out,
                    &mut word,
                    &mut has_word,
                    &mut want_delim,
                    &mut pending,
                );
                out.push(Tok::Op(format!("{ch}{ch}")));
                i += 2;
            }
            '|' => {
                flush(
                    &mut out,
                    &mut word,
                    &mut has_word,
                    &mut want_delim,
                    &mut pending,
                );
                out.push(Tok::Op("|".to_owned()));
                i += 1;
            }
            '<' | '>' => {
                flush(
                    &mut out,
                    &mut word,
                    &mut has_word,
                    &mut want_delim,
                    &mut pending,
                );
                let mut op = String::new();
                while i < c.len() && c[i] == ch {
                    op.push(ch);
                    i += 1;
                }
                if op == "<<" && c.get(i) == Some(&'-') {
                    op.push('-');
                    i += 1;
                }
                want_delim = op == "<<" || op == "<<-";
                out.push(Tok::Op(op));
            }
            _ => {
                word.push(ch);
                has_word = true;
                i += 1;
            }
        }
    }
    flush(
        &mut out,
        &mut word,
        &mut has_word,
        &mut want_delim,
        &mut pending,
    );
    out
}

/// One simple command: its words, where it redirects output to, and whether it reads a heredoc.
#[derive(Default)]
struct Segment {
    words: Vec<String>,
    outputs: Vec<String>,
    heredoc: bool,
}

fn segments(tokens: Vec<Tok>) -> Vec<Segment> {
    let mut all = Vec::new();
    let mut cur = Segment::default();
    let mut it = tokens.into_iter().peekable();
    while let Some(tok) = it.next() {
        match tok {
            Tok::Word(w) => cur.words.push(w),
            Tok::Op(op) if matches!(op.as_str(), ";" | "&&" | "||" | "|") => {
                all.push(std::mem::take(&mut cur));
            }
            Tok::Op(op) => {
                cur.heredoc |= op == "<<" || op == "<<-";
                // The word after a redirection or a heredoc is its target or delimiter.
                if let Some(Tok::Word(_)) = it.peek()
                    && let Some(Tok::Word(target)) = it.next()
                    && op.starts_with('>')
                {
                    cur.outputs.push(target);
                }
            }
        }
    }
    all.push(cur);
    all
}

/// Whether a write to `target` is the agent's own business: a `scratchpad` folder, a device, a
/// file descriptor, a shell variable (not known here, so not judged) or one of `roots`.
fn is_scratch(target: &str, roots: &[String]) -> bool {
    let t = target.trim();
    if t.is_empty() || t == "-" || t.starts_with(['&', '$']) || t.starts_with("/dev/") {
        return true;
    }
    if t.split(['/', '\\']).any(|part| part == "scratchpad") {
        return true;
    }
    roots.iter().any(|root| {
        let root = root.trim_end_matches(['/', '\\']);
        !root.is_empty()
            && t.strip_prefix(root)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '\\']))
    })
}

fn program(name: &str) -> &str {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    base.strip_suffix(".exe").unwrap_or(base)
}

/// `sed -i`, `sed --in-place`, `sed -i.bak`, `sed -Ei`.
fn sed_in_place(arg: &str) -> bool {
    arg.starts_with("--in-place")
        || (arg.starts_with('-') && !arg.starts_with("--") && arg[1..].contains('i'))
}

/// `perl -i`, `perl -pi`, `perl -pi.bak`, `perl -lpi`; not `-Mstrict` or `-e`.
fn perl_in_place(arg: &str) -> bool {
    let Some(flags) = arg.strip_prefix('-') else {
        return false;
    };
    let lead = flags.trim_start_matches([
        'n', 'l', 'a', 'p', 's', 'w', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    ]);
    lead.starts_with('i')
}

fn edits_a_file(command: &str, scratch: &[String], depth: u8) -> bool {
    segments(tokenize(command))
        .iter()
        .any(|seg| segment_edits(seg, scratch, depth))
}

fn segment_edits(seg: &Segment, scratch: &[String], depth: u8) -> bool {
    // A heredoc redirected into a file that is not scratch.
    if seg.heredoc && seg.outputs.iter().any(|t| !is_scratch(t, scratch)) {
        return true;
    }
    let mut words = seg.words.iter().map(String::as_str).peekable();
    while let Some(w) = words.peek() {
        let env_assignment = w.contains('=') && !w.starts_with('-') && !w.starts_with('=');
        if env_assignment
            || matches!(
                program(w),
                "env" | "command" | "time" | "exec" | "sudo" | "nohup"
            )
        {
            words.next();
        } else {
            break;
        }
    }
    let Some(prog) = words.next().map(program) else {
        return false;
    };
    let args: Vec<&str> = words.collect();
    let files = || args.iter().filter(|a| !a.starts_with('-'));
    match prog {
        "sed" => {
            args.iter().any(|a| sed_in_place(a))
                && files().next_back().is_some_and(|f| !is_scratch(f, scratch))
        }
        "perl" => {
            seg.heredoc
                || (args.iter().any(|a| perl_in_place(a))
                    && files().next_back().is_some_and(|f| !is_scratch(f, scratch)))
        }
        "tee" => files().any(|f| !is_scratch(f, scratch)),
        // `find ... -exec sed -i {} +`: judge the command that find runs, on a file it found.
        "find" if depth < 3 => args.iter().enumerate().any(|(p, a)| {
            matches!(*a, "-exec" | "-execdir" | "-ok" | "-okdir") && {
                let run = args[p + 1..]
                    .iter()
                    .take_while(|w| !matches!(**w, ";" | "+"))
                    .map(|w| (*w).to_owned())
                    .collect();
                let inner = Segment {
                    words: run,
                    ..Segment::default()
                };
                segment_edits(&inner, scratch, depth + 1)
            }
        }),
        // awk rewrites a file by redirecting into a temp file and moving it over the original.
        "awk" | "gawk" | "mawk" | "nawk" => {
            seg.outputs.iter().any(|t| !is_scratch(t, scratch))
                || args.windows(2).any(|w| w[0] == "-i" && w[1] == "inplace")
        }
        "python" | "python3" | "py" => seg.heredoc,
        "sh" | "bash" | "zsh" if depth < 3 => args
            .iter()
            .position(|a| *a == "-c")
            .and_then(|p| args.get(p + 1))
            .is_some_and(|inner| edits_a_file(inner, scratch, depth + 1)),
        _ => false,
    }
}

#[cfg(test)]
#[path = "shell_edit_tests.rs"]
mod tests;
