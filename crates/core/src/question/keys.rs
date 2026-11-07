//! Named keys in a free-text answer: `{down}{enter}` sends the Down arrow and Enter, for dialogs
//! whose options are not numbered. A text without any named key is typed as it is.

/// The names a text may use, and the bytes each sends.
const KEYS: [(&str, &[u8]); 8] = [
    ("up", b"\x1b[A"),
    ("down", b"\x1b[B"),
    ("right", b"\x1b[C"),
    ("left", b"\x1b[D"),
    ("enter", b"\r"),
    ("esc", b"\x1b"),
    ("tab", b"\t"),
    ("space", b" "),
];

fn named(name: &str) -> Option<&'static [u8]> {
    let name = name.to_ascii_lowercase();
    KEYS.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}

/// Whether the text contains at least one named key such as `{down}`.
pub fn has_named_keys(text: &str) -> bool {
    expand(text).1
}

/// The bytes the text stands for: named keys become their key, everything else is typed.
pub fn expand_keys(text: &str) -> Vec<u8> {
    expand(text).0
}

fn expand(text: &str) -> (Vec<u8>, bool) {
    let mut out = Vec::new();
    let mut found = false;
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let (before, tail) = rest.split_at(open);
        out.extend_from_slice(before.as_bytes());
        let key = tail
            .find('}')
            .and_then(|close| named(&tail[1..close]).map(|b| (b, close)));
        match key {
            Some((bytes, close)) => {
                out.extend_from_slice(bytes);
                found = true;
                rest = &tail[close + 1..];
            }
            None => {
                out.push(b'{');
                rest = &tail[1..];
            }
        }
    }
    out.extend_from_slice(rest.as_bytes());
    (out, found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_become_their_bytes_and_other_text_is_kept() {
        assert_eq!(expand_keys("{down}{Enter}"), b"\x1b[B\r");
        assert_eq!(expand_keys("a{up}b"), b"a\x1b[Ab");
        assert!(has_named_keys("{esc}"));
    }

    #[test]
    fn braces_that_are_not_keys_stay_text() {
        assert!(!has_named_keys("{nope} {"));
        assert_eq!(expand_keys("{nope} {"), b"{nope} {");
    }
}
