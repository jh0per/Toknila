//! The bounded scrollback of one run: the last N lines of its raw output. Offsets are absolute
//! (bytes since the run began), so they match the run's log file and a client can continue a
//! stream from where a scrollback answer ended.

/// The tail of a run's output, at most `max_lines` newline-terminated lines (plus the unfinished
/// line being written).
#[derive(Debug)]
pub struct Scrollback {
    buf: Vec<u8>,
    /// Offset of `buf[0]` in the run's output.
    start: u64,
    lines: usize,
    max_lines: usize,
}

impl Scrollback {
    /// An empty scrollback that keeps `max_lines` lines (at least one).
    pub fn new(max_lines: usize) -> Self {
        Self::starting_at(max_lines, 0)
    }

    /// Like [`Scrollback::new`] for output whose first byte is at `offset` of the run.
    pub fn starting_at(max_lines: usize, offset: u64) -> Self {
        Self {
            buf: Vec::new(),
            start: offset,
            lines: 0,
            max_lines: max_lines.max(1),
        }
    }

    /// Appends output and forgets the oldest lines beyond the limit.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
        self.lines += bytes.iter().filter(|b| **b == b'\n').count();
        if self.lines > self.max_lines {
            self.trim(self.lines - self.max_lines);
        }
    }

    /// Drops everything up to and including the `n`th newline.
    fn trim(&mut self, n: usize) {
        let cut = self
            .buf
            .iter()
            .enumerate()
            .filter(|(_, b)| **b == b'\n')
            .nth(n - 1)
            .map_or(self.buf.len(), |(i, _)| i + 1);
        self.buf.drain(..cut);
        self.start += cut as u64;
        self.lines -= n;
    }

    /// Offset after the last byte pushed.
    pub fn end(&self) -> u64 {
        self.start + self.buf.len() as u64
    }

    /// The bytes from `offset` on, with the offset they really start at: later than asked when
    /// the lines before it were dropped.
    pub fn from(&self, offset: u64) -> (u64, &[u8]) {
        let at = offset.clamp(self.start, self.end());
        (
            at,
            &self.buf[usize::try_from(at - self.start).unwrap_or(0)..],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &Scrollback, from: u64) -> (u64, String) {
        let (at, bytes) = s.from(from);
        (at, String::from_utf8_lossy(bytes).into_owned())
    }

    #[test]
    fn keeps_the_last_lines_and_counts_offsets_from_the_start_of_the_run() {
        let mut s = Scrollback::new(2);
        s.push(b"one\ntwo\n");
        assert_eq!(text(&s, 0), (0, "one\ntwo\n".to_owned()));
        s.push(b"three\nfo");
        // "one\n" went; "four" is the unfinished line.
        assert_eq!(text(&s, 0), (4, "two\nthree\nfo".to_owned()));
        assert_eq!(s.end(), 16);
    }

    #[test]
    fn a_burst_larger_than_the_limit_is_cut_at_a_line_boundary() {
        let mut s = Scrollback::new(2);
        s.push(b"a\nb\nc\nd\ne\n");
        assert_eq!(text(&s, 0), (6, "d\ne\n".to_owned()));
    }

    #[test]
    fn a_later_offset_continues_where_the_client_stopped() {
        let mut s = Scrollback::new(10);
        s.push(b"one\ntwo\n");
        assert_eq!(text(&s, 4), (4, "two\n".to_owned()));
        assert_eq!(text(&s, 99), (8, String::new()));
    }

    #[test]
    fn a_scrollback_can_start_at_an_offset_of_the_run() {
        let mut s = Scrollback::starting_at(10, 100);
        s.push(b"one\n");
        assert_eq!(text(&s, 0), (100, "one\n".to_owned()));
        assert_eq!(s.end(), 104);
    }

    #[test]
    fn the_limit_is_at_least_one_line() {
        let mut s = Scrollback::new(0);
        s.push(b"a\nb\n");
        assert_eq!(text(&s, 0), (2, "b\n".to_owned()));
    }
}
