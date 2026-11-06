//! Knowing that the last output of a process has been read.
//!
//! On macOS the kernel throws away what a terminal has not yet delivered when its last slave end
//! closes, so a process that prints and exits at once can lose its output if the daemon closed its
//! own slave end (and the reader had not run yet). The daemon therefore keeps a slave end open for
//! the whole run, which means the reader no longer sees the end of the terminal when the process
//! exits. Instead, once the process has ended, the daemon writes a [`MARK`] through that end: the
//! terminal delivers it after everything the process printed, so the reader seeing it knows the
//! output is complete, with no pause to guess with.

use std::path::Path;

/// Written after the process has ended; never part of the output.
pub(super) const MARK: &[u8] = b"\0toknila-drained\0";

/// Passes output on and spots the [`MARK`], even when it arrives split between two reads.
#[derive(Debug, Default)]
pub(super) struct MarkFilter {
    /// The end of the last read that may be the start of the mark, held back until it is known.
    held: Vec<u8>,
}

impl MarkFilter {
    /// The bytes of `chunk` (and of earlier ones held back) that are output, and whether the mark
    /// was seen: nothing after it is output.
    pub fn feed(&mut self, chunk: &[u8]) -> (Vec<u8>, bool) {
        let mut data = std::mem::take(&mut self.held);
        data.extend_from_slice(chunk);
        if let Some(at) = data.windows(MARK.len()).position(|w| w == MARK) {
            data.truncate(at);
            return (data, true);
        }
        let keep = (1..MARK.len())
            .rev()
            .find(|&k| data.ends_with(&MARK[..k]))
            .unwrap_or(0);
        self.held = data.split_off(data.len() - keep);
        (data, false)
    }

    /// What is still held back when the terminal ends without the mark: it was output after all.
    pub fn rest(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.held)
    }
}

/// A slave end of the terminal kept by the daemon, to write the [`MARK`] through. Where there is
/// none (Windows), the reader ends with the terminal instead.
#[derive(Debug, Default)]
pub(super) struct Slave(Option<std::fs::File>);

impl Slave {
    /// Opens the slave end at `path`. Never blocks and never becomes the daemon's own terminal.
    #[cfg(unix)]
    pub fn open(path: Option<&Path>) -> Slave {
        use std::os::unix::fs::OpenOptionsExt;
        let flags = nix::fcntl::OFlag::O_NOCTTY | nix::fcntl::OFlag::O_NONBLOCK;
        let file = path.and_then(|p| {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(flags.bits())
                .open(p)
                .ok()
        });
        Slave(file)
    }

    #[cfg(not(unix))]
    pub fn open(_: Option<&Path>) -> Slave {
        Slave(None)
    }

    /// Writes the mark after the output; `false` if it could not be (the reader then ends by its
    /// own timeout).
    pub fn mark(&mut self) -> bool {
        use std::io::Write;
        self.0
            .as_mut()
            .is_some_and(|f| f.write_all(MARK).and_then(|()| f.flush()).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(chunks: &[&[u8]]) -> (Vec<u8>, bool) {
        let mut f = MarkFilter::default();
        let mut out = Vec::new();
        for c in chunks {
            let (bytes, found) = f.feed(c);
            out.extend(bytes);
            if found {
                return (out, true);
            }
        }
        out.extend(f.rest());
        (out, false)
    }

    #[test]
    fn output_before_the_mark_is_kept_and_what_follows_is_not() {
        let mut chunk = b"hello".to_vec();
        chunk.extend_from_slice(MARK);
        chunk.extend_from_slice(b"later");
        assert_eq!(all(&[&chunk]), (b"hello".to_vec(), true));
    }

    #[test]
    fn a_mark_split_between_reads_is_found_at_every_split() {
        for cut in 1..MARK.len() {
            let (a, b) = MARK.split_at(cut);
            let mut first = b"out".to_vec();
            first.extend_from_slice(a);
            assert_eq!(all(&[&first, b]), (b"out".to_vec(), true), "cut at {cut}");
        }
    }

    #[test]
    fn a_false_start_of_the_mark_is_output_when_the_terminal_ends_without_it() {
        assert_eq!(all(&[b"a\0toknila"]), (b"a\0toknila".to_vec(), false));
        assert_eq!(all(&[b"a\0tok", b"nope"]), (b"a\0toknope".to_vec(), false));
    }

    #[test]
    fn plain_output_passes_through_whole() {
        assert_eq!(all(&[b"one ", b"two"]), (b"one two".to_vec(), false));
    }
}
