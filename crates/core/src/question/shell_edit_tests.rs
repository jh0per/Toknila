use super::*;

fn denied(command: &str) -> bool {
    shell_edit_denial(&format!("Bash: {command}"), &[]).is_some()
}

/// The command of TOKNILA-58 that slipped past the refusal, word for word.
const TOKNILA_58: &str = "cd <ship>/apps/web && sed -i '' 's/item\\.ticket/item.card/g; s/CardItem/InboxItem/g' src/views/Answer.tsx";

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

/// Near neighbours of TOKNILA-58: the same edit in another spelling, after a `cd`, or by another
/// tool.
const NEIGHBOURS: [&str; 12] = [
    TOKNILA_58,
    "cd \"/ships/x/apps/web\" && sed -i '' 's/a/b/g' src/views/Answer.tsx",
    "cd /ships/x && sed -i.bak 's/a/b/' src/lib.rs",
    "sed -i'' -e 's/a/b/' src/lib.rs",
    "sed --in-place 's/a/b/' src/lib.rs",
    "perl -pi -e 's/a/b/' src/lib.rs",
    "perl -i.bak -pe 's/a/b/' src/lib.rs",
    "awk '{ print $1 }' src/lib.rs > tmp && mv tmp src/lib.rs",
    "python3 - <<'EOF'\nopen('src/lib.rs', 'w').write('x')\nEOF",
    "cd crates/core && cat > src/x.rs <<EOF\nfn x() {}\nEOF",
    "find src -name '*.rs' -exec sed -i 's/a/b/' {} +",
    "find src -name '*.rs' -exec sed -i '' 's/a/b/' {} +",
];

/// Reads, builds and writes into the agent's own scratchpad.
const FINE: [&str; 20] = [
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
    "sed -n '1,20p' src/lib.rs",
    "sed 's/a/b/' src/lib.rs",
    "cd /ships/x && sed 's/a/b/' src/lib.rs | head",
    "cd /ships/x/apps/web && grep -n 'item' src/views/Answer.tsx",
    "cd /ships/x && cat src/lib.rs",
    "cd /ships/x && sed -n '1,5p' src/lib.rs > /private/tmp/claude-501/p/s/scratchpad/head.txt",
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
fn the_command_of_toknila_58_is_denied() {
    assert_eq!(
        shell_edit_denial(&format!("Bash: {TOKNILA_58}"), &[]),
        Some(Decision::Deny {
            message: SHELL_EDIT_DENIAL.to_owned()
        })
    );
}

#[test]
fn near_neighbours_of_toknila_58_are_denied() {
    for c in NEIGHBOURS {
        assert!(denied(c), "{c}");
    }
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
