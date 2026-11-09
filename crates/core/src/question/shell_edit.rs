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
mod tests {
    use super::*;

    fn denied(command: &str) -> bool {
        shell_edit_denial(&format!("Bash: {command}"), &[]).is_some()
    }

    /// Commands of the shape the agents of runs 3 and 4 sent first (see
    /// `docs/runs/claude-code-e2e-3.md` and `-4.md`): an interpreter or an editor through a shell.
    const EDITS: [&str; 14] = [
        "python3 - <<'EOF'\nfrom pathlib import Path\np = Path('crates/core/src/usage/price.rs')\np.write_text(p.read_text().replace('a', 'b'))\nEOF",
        "python3 - <<EOF\nprint(1)\nEOF",
        "cd \"/ships/x\" && python3 - <<'PY'\nopen('a.rs','w').write('x')\nPY",
        "cat > crates/core/src/parse.rs <<'EOF'\nfn main() {}\nEOF",
        "cat >> crates/core/src/lib.rs <<EOF\npub mod x;\nEOF",
        "cat <<'EOF' > crates/cli/src/cmd/hook.rs\nfn x() {}\nEOF",
        "sed -i 's/foo/bar/' crates/core/src/usage/price.rs",
        "sed -i '' 's/foo/bar/' crates/core/src/usage/price.rs",
        "sed -i.bak -e 's/>/>=/' crates/core/src/usage/price.rs && cargo test -p toknila-core",
        "sed --in-place 's/a/b/' Cargo.toml",
        "perl -pi -e 's/foo/bar/g' crates/core/src/parse.rs",
        "perl -pi.bak -e 's/foo/bar/g' crates/core/src/parse.rs",
        "echo 'mod x;' | tee -a crates/core/src/lib.rs",
        "export PATH=\"$HOME/.cargo/bin:$PATH\"; cd \"/ships/x\" && sed -i 's/a/b/' src/lib.rs",
    ];

    /// Reads, builds and writes into the agent's own scratchpad.
    const FINE: [&str; 14] = [
        "cat crates/core/src/lib.rs",
        "sed -n '1,40p' crates/core/src/hook.rs",
        "sed -n '/fn main/,/^}/p' src/main.rs | head",
        "grep -rn \"permission\" crates --include=\"*.rs\"",
        "cargo xtask verify",
        "git diff --cached | cat",
        "cat > /private/tmp/claude-501/p/s/scratchpad/notes.md <<'EOF'\nhello\nEOF",
        "cat <<EOF > \"/tmp/claude-501/p/s/scratchpad/in.json\"\n{}\nEOF",
        "sed -i 's/a/b/' /private/tmp/claude-501/p/s/scratchpad/copy.rs",
        "cat <<'EOF'\nnothing is written\nEOF",
        "cargo test -p toknila-core > /dev/null 2>&1",
        "ls -la | tee /private/tmp/claude-501/p/s/scratchpad/ls.txt",
        "git commit -m \"$(cat <<'EOF'\nsed -i is not run here\nEOF\n)\"",
        "echo \"sed -i x file\"",
    ];

    #[test]
    fn shell_edits_of_the_real_runs_are_denied_with_the_reason() {
        for c in EDITS {
            let got = shell_edit_denial(&format!("Bash: {c}"), &[]);
            assert_eq!(
                got,
                Some(Decision::Deny {
                    message: SHELL_EDIT_DENIAL.to_owned()
                }),
                "{c}"
            );
        }
        assert!(SHELL_EDIT_DENIAL.contains("Edit") && SHELL_EDIT_DENIAL.contains("Write"));
    }

    #[test]
    fn reads_builds_and_scratchpad_writes_are_not_touched() {
        for c in FINE {
            assert!(!denied(c), "{c}");
        }
    }

    #[test]
    fn only_bash_calls_are_judged() {
        assert_eq!(shell_edit_denial("Write: /w/a.rs", &[]), None);
        assert_eq!(shell_edit_denial("Edit: sed -i x", &[]), None);
        assert_eq!(shell_edit_denial("mcp__x__y", &[]), None);
    }

    #[test]
    fn a_shell_wrapper_does_not_hide_the_edit() {
        assert!(denied("bash -c \"sed -i 's/a/b/' src/lib.rs\""));
        assert!(!denied("bash -c \"sed -n 1p src/lib.rs\""));
    }

    #[test]
    fn extra_scratch_folders_are_free_to_write() {
        let roots = vec!["/var/folders/zz/T".to_owned()];
        let c = "Bash: cat > /var/folders/zz/T/x.txt <<EOF\nhi\nEOF";
        assert_eq!(shell_edit_denial(c, &roots), None);
        assert!(shell_edit_denial(c, &[]).is_some());
        // A sibling folder with the same prefix is not inside the root.
        let c = "Bash: cat > /var/folders/zz/Tx/x.txt <<EOF\nhi\nEOF";
        assert!(shell_edit_denial(c, &roots).is_some());
    }
}
