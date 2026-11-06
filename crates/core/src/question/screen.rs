//! The visible text of a terminal's output: escape sequences removed, the last lines kept.

use std::iter::Peekable;
use std::str::Chars;

/// The most lines of an agent's screen the inbox carries.
pub const SCREEN_LINES: usize = 15;

/// The last `max` non-empty lines of raw terminal `output`, with ANSI escape sequences removed.
/// Cursor moves that start a new row (`H`, `f`, `B`, `E`, `d`) break the line and a move to the
/// right (`C`) leaves a space, so a full-screen program that never prints a newline still reads
/// line by line. Invalid UTF-8 is replaced.
pub fn last_lines(output: &[u8], max: usize) -> Vec<String> {
    let text = String::from_utf8_lossy(output);
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\x1b' => skip_escape(&mut chars, &mut plain),
            '\r' => {
                // A bare carriage return redraws the line; "\r\n" is an ordinary break.
                if chars.peek() != Some(&'\n') {
                    plain.push('\n');
                }
            }
            c if c == '\n' || c == '\t' || !c.is_control() => plain.push(c),
            _ => {}
        }
    }
    let lines: Vec<String> = plain
        .lines()
        .map(|l| l.trim_end().to_owned())
        .filter(|l| !l.trim().is_empty())
        .collect();
    let from = lines.len().saturating_sub(max);
    lines[from..].to_vec()
}

fn skip_escape(chars: &mut Peekable<Chars<'_>>, out: &mut String) {
    match chars.next() {
        // CSI: parameters, then a final byte in @..~
        Some('[') => {
            for c in chars.by_ref() {
                if ('\x40'..='\x7e').contains(&c) {
                    match c {
                        'H' | 'f' | 'B' | 'E' | 'd' => out.push('\n'),
                        'C' => out.push(' '),
                        _ => {}
                    }
                    return;
                }
            }
        }
        // OSC and other strings: up to BEL or ST (ESC \).
        Some(']' | 'P' | '^' | '_') => {
            while let Some(c) = chars.next() {
                if c == '\x07' {
                    return;
                }
                if c == '\x1b' {
                    chars.next();
                    return;
                }
            }
        }
        // A character-set selection such as ESC ( B.
        Some('(' | ')' | '*' | '+') => {
            chars.next();
        }
        _ => {}
    }
}
