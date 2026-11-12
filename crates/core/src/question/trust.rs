//! Claude Code's folder trust dialog, the first screen of every new project folder: how to tell
//! it from another screen and which keys choose one of its two options.

/// The dialog's option that trusts the folder.
pub const TRUST_YES: &str = "Yes, I trust this folder";
/// The dialog's option that exits.
pub const TRUST_NO: &str = "No, exit";

/// The marks a menu puts in front of its highlighted row.
const POINTERS: [char; 3] = ['>', '❯', '›'];

/// The rows of the two options in `lines` (the last occurrence of each), as (yes, no).
fn rows(lines: &[String]) -> Option<(usize, usize)> {
    let last = |text: &str| lines.iter().rposition(|l| l.contains(text));
    Some((last(TRUST_YES)?, last(TRUST_NO)?))
}

/// Whether the screen is the trust dialog: both of its options are shown.
pub fn is_trust_dialog(lines: &[String]) -> bool {
    rows(lines).is_some()
}

/// The keys that choose Yes (`yes`) or No on the trust dialog shown in `lines`: the arrow from the
/// highlighted option to the chosen one, then Enter. `None` when the screen is not the dialog.
/// A highlight that cannot be seen is taken to be the first option.
pub fn trust_keys(lines: &[String], yes: bool) -> Option<Vec<u8>> {
    let (yes_row, no_row) = rows(lines)?;
    let (target, other) = if yes {
        (yes_row, no_row)
    } else {
        (no_row, yes_row)
    };
    let pointed = |row: usize| lines[row].trim_start().starts_with(POINTERS);
    let highlighted = if pointed(yes_row) {
        yes_row
    } else if pointed(no_row) {
        no_row
    } else {
        yes_row.min(no_row)
    };
    let mut keys = Vec::new();
    if highlighted != target {
        keys.extend_from_slice(if target > other { b"\x1b[B" } else { b"\x1b[A" });
    }
    keys.push(b'\r');
    Some(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(rows: &[&str]) -> Vec<String> {
        rows.iter().map(|r| (*r).to_owned()).collect()
    }

    /// The dialog as Claude Code draws it: "No, exit" first and highlighted.
    fn dialog() -> Vec<String> {
        lines(&[
            "Is this a project you created or one you trust?",
            "❯ No, exit",
            "  Yes, I trust this folder",
        ])
    }

    #[test]
    fn the_dialog_is_told_from_another_screen() {
        assert!(is_trust_dialog(&dialog()));
        assert!(!is_trust_dialog(&lines(&[
            "Log in to continue",
            "> 1. Browser"
        ])));
        assert!(!is_trust_dialog(&lines(&["Yes, I trust this folder"])));
    }

    #[test]
    fn yes_is_the_arrow_to_the_option_below_then_enter_and_no_is_enter_on_the_highlight() {
        assert_eq!(trust_keys(&dialog(), true), Some(b"\x1b[B\r".to_vec()));
        assert_eq!(trust_keys(&dialog(), false), Some(b"\r".to_vec()));
    }

    #[test]
    fn the_keys_follow_the_highlight_and_the_order_of_the_options() {
        let yes_first = lines(&["> 1. Yes, I trust this folder", "  2. No, exit"]);
        assert_eq!(trust_keys(&yes_first, true), Some(b"\r".to_vec()));
        assert_eq!(trust_keys(&yes_first, false), Some(b"\x1b[B\r".to_vec()));
        let no_below = lines(&["  Yes, I trust this folder", "> No, exit"]);
        assert_eq!(trust_keys(&no_below, true), Some(b"\x1b[A\r".to_vec()));
        assert_eq!(trust_keys(&lines(&["Something else"]), true), None);
    }
}
