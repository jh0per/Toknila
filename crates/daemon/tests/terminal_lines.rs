//! TOKNILA-162 probe, layer "terminal": which of 80 numbered lines a program prints reach the
//! reader of its terminal, with no daemon code in between. The program prints them as fast as it
//! can (one write of all 80, or 80 writes with no pause, or paced), at the runner's screen size and
//! at a taller one, with the cursor query answered at once or late (a busy daemon), with the screen
//! shrunk during the output, and resized after it.
//!
//! On Unix a pty hands the reader the program's bytes as they are, so every case gets all 80: the
//! control that the counting works. Windows' ConPTY is not a byte pipe: it keeps a screen, renders
//! it and emits VT for what it rendered. Fast output still reaches the reader whole; a shrink
//! early in the output can drop the first lines; a resize repaints the screen from its top row.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

mod support;

const AGENT: &str = env!("CARGO_BIN_EXE_fake-agent");
/// How many numbered lines the program prints.
const LINES: u32 = 80;
/// The line printed after them; once the reader has it, nothing earlier is still to come.
const END: &str = "burst over";
const WAIT: Duration = Duration::from_secs(20);
/// Runs of each case.
const REPS: usize = 5;

/// How the program prints its lines.
#[derive(Debug, Clone, Copy)]
enum Print {
    /// All of them in one write.
    Burst,
    /// One write per line, no pause between them.
    EachLine,
    /// One write per line, two milliseconds apart: still printing when a panel's size comes.
    Paced,
}

#[derive(Debug, Clone, Copy)]
struct Case {
    print: Print,
    rows: u16,
    /// How long after the cursor query the answer is written (ConPTY asks once, at its start).
    answer_after: Duration,
    /// When the screen shrinks to 152x14 (a panel's size), counted from the cursor answer (from
    /// the start on Unix, which asks nothing); `None`: never.
    shrink_after: Option<Duration>,
}

/// `bytes` as text without the terminal's escape sequences (CSI, OSC and two-byte ones).
fn plain(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('\x40'..='\x7e').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\x07' || (c == '\x1b' && chars.next_if_eq(&'\\').is_some()) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The numbers N of the lines "scroll line N" in `text` (whole numbers: "scroll line 1" is not
/// the start of "scroll line 10").
fn numbers(text: &str) -> BTreeSet<u32> {
    text.match_indices("scroll line ")
        .filter_map(|(at, m)| {
            let rest = &text[at + m.len()..];
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .filter(|n| (1..=LINES).contains(n))
        .collect()
}

/// The program's arguments for `print`.
fn steps(print: Print) -> Vec<String> {
    let mut steps = vec!["sleep".to_owned(), "toknila-162-probe".to_owned()];
    match print {
        Print::Burst => steps.extend(["burst".into(), LINES.to_string(), "scroll line".into()]),
        Print::EachLine => {
            for i in 1..=LINES {
                steps.extend(["print".to_owned(), format!("scroll line {i}")]);
            }
        }
        Print::Paced => {
            for i in 1..=LINES {
                steps.extend(["print".to_owned(), format!("scroll line {i}")]);
                steps.extend(["sleep".to_owned(), "2".to_owned()]);
            }
        }
    }
    // It holds until killed: on macOS a terminal whose last slave end closes drops what it has not
    // delivered yet, which would be a loss of the probe's own making.
    steps.extend(["print".to_owned(), END.to_owned(), "hold".to_owned()]);
    steps
}

/// A program in a terminal of 160 columns, and what its reader got so far.
struct Term {
    master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    rx: mpsc::Receiver<Vec<u8>>,
    got: Vec<u8>,
    opened: Instant,
    asked: Option<Instant>,
    answered: Option<Instant>,
}

impl Term {
    fn open(rows: u16, steps: Vec<String>) -> Term {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols: 160,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty");
        let mut cmd = CommandBuilder::new(AGENT);
        cmd.args(steps);
        let child = pair.slave.spawn_command(cmd).expect("spawn");
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().expect("reader");
        let writer = pair.master.take_writer().expect("writer");
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        Term {
            master: pair.master,
            child,
            writer,
            rx,
            got: Vec::new(),
            opened: Instant::now(),
            asked: None,
            answered: None,
        }
    }

    /// Takes what arrives within a millisecond, and answers ConPTY's cursor query `answer_after`
    /// it came.
    fn pump(&mut self, answer_after: Duration) {
        if let Ok(chunk) = self.rx.recv_timeout(Duration::from_millis(1)) {
            self.got.extend_from_slice(&chunk);
        }
        if self.asked.is_none() && self.got.windows(4).any(|w| w == b"\x1b[6n") {
            self.asked = Some(Instant::now());
        }
        if self.answered.is_none() && self.asked.is_some_and(|t| t.elapsed() >= answer_after) {
            self.answered = Some(Instant::now());
            self.writer.write_all(b"\x1b[1;1R").expect("answer");
        }
    }

    fn resize(&self, cols: u16, rows: u16) {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("resize");
    }

    /// Reads until the program's last line has arrived (or [`WAIT`] has passed); shrinks the
    /// screen to 152x14 when the case says so, counted from the cursor answer (Unix: the start).
    fn until_end(&mut self, case: Case) {
        let until = Instant::now() + WAIT;
        let mut shrink = case.shrink_after;
        while Instant::now() < until && !plain(&self.got).contains(END) {
            self.pump(case.answer_after);
            let from = if cfg!(windows) {
                self.answered
            } else {
                Some(self.opened)
            };
            if let Some(after) = shrink
                && from.is_some_and(|t| t.elapsed() >= after)
            {
                self.resize(152, 14);
                shrink = None;
            }
        }
        let text = plain(&self.got);
        assert!(
            text.contains(END),
            "the last line never reached the reader ({} bytes): {:?}",
            self.got.len(),
            text.chars().rev().take(300).collect::<String>()
        );
    }

    /// What arrives in the next `window`.
    fn next_for(&mut self, window: Duration) -> Vec<u8> {
        let from = self.got.len();
        let until = Instant::now() + window;
        while Instant::now() < until {
            self.pump(Duration::ZERO);
        }
        self.got[from..].to_vec()
    }

    fn close(mut self) {
        let _ = self.child.kill();
        drop(self.writer);
        drop(self.master);
    }
}

/// One run of `case`: the numbers of the lines the reader got.
fn run(case: Case) -> BTreeSet<u32> {
    let mut term = Term::open(case.rows, steps(case.print));
    term.until_end(case);
    let text = plain(&term.got);
    term.close();
    numbers(&text)
}

/// `missing` as ranges: "1-5, 70".
fn ranges(missing: &[u32]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < missing.len() {
        let mut j = i;
        while j + 1 < missing.len() && missing[j + 1] == missing[j] + 1 {
            j += 1;
        }
        out.push(if i == j {
            missing[i].to_string()
        } else {
            format!("{}-{}", missing[i], missing[j])
        });
        i = j + 1;
    }
    out.join(", ")
}

/// One line of the report: how many lines each run got, and which ones the worst run missed.
fn report(case: Case, runs: &[BTreeSet<u32>]) -> String {
    let counts: Vec<String> = runs.iter().map(|r| r.len().to_string()).collect();
    let worst = runs.iter().min_by_key(|r| r.len()).expect("a run");
    let missing: Vec<u32> = (1..=LINES).filter(|n| !worst.contains(n)).collect();
    let shrink = case
        .shrink_after
        .map_or("never".to_owned(), |d| format!("{}ms", d.as_millis()));
    format!(
        "{:?} rows={} answer_after={}ms shrink_after={shrink}: got [{}] of {LINES}; worst missed [{}]",
        case.print,
        case.rows,
        case.answer_after.as_millis(),
        counts.join(" "),
        ranges(&missing)
    )
}

/// Runs each case [`REPS`] times and prints a report line per case; every run's lines, and the
/// report.
fn probe(cases: &[Case]) -> (Vec<BTreeSet<u32>>, String) {
    let mut lines = Vec::new();
    let mut all = Vec::new();
    for &case in cases {
        let runs: Vec<BTreeSet<u32>> = (0..REPS).map(|_| run(case)).collect();
        let line = report(case, &runs);
        eprintln!("TOKNILA-162 probe: {line}");
        lines.push(line);
        all.extend(runs);
    }
    (all, lines.join("\n"))
}

/// Whether a run got all [`LINES`] lines.
fn whole(run: &BTreeSet<u32>) -> bool {
    run.len() == LINES as usize
}

/// The first numbered line a repaint draws at the top of the screen (after its last `ESC [ H`).
fn top_line(repaint: &[u8]) -> Option<u32> {
    let home = repaint.windows(3).rposition(|w| w == b"\x1b[H")?;
    numbers(plain(&repaint[home..]).lines().next()?)
        .into_iter()
        .next()
}

#[test]
fn the_escape_filter_and_the_counter_find_whole_numbered_lines() {
    let bytes = b"\x1b[?25l\x1b[2J\x1b[Hscroll line 1\x1b[K\r\nscroll \x1b[mline 2\r\n\x1b]0;t\x07scroll line 10\r\nscroll line 81";
    // A line split by a sequence counts; 81 is past the program's lines.
    assert_eq!(numbers(&plain(bytes)), BTreeSet::from([1, 2, 10]));
    assert_eq!(ranges(&[1, 2, 3, 5, 70, 71]), "1-3, 5, 70-71");
}

/// Fast output alone loses nothing, on every system: ConPTY renders before a line would scroll
/// off its screen. On GitHub's Windows runner every case got 80 of 80 in all five runs, twice (CI
/// runs 44308344208 and 44317344931), the cursor answered at once or 300 ms late.
#[test]
fn every_line_of_a_fast_program_reaches_the_terminal_reader() {
    let _turn = support::hold();
    let late = Duration::from_millis(300);
    let cases = [
        (Print::Burst, 40, Duration::ZERO),
        (Print::Burst, 200, Duration::ZERO),
        (Print::EachLine, 40, Duration::ZERO),
        (Print::EachLine, 40, late),
        (Print::EachLine, 200, late),
        (Print::Burst, 40, late),
        (Print::Burst, 200, late),
    ]
    .map(|(print, rows, answer_after)| Case {
        print,
        rows,
        answer_after,
        shrink_after: None,
    });
    let (runs, report) = probe(&cases);
    assert!(
        runs.iter().all(whole),
        "the terminal reader lost lines:\n{report}"
    );
}

/// A screen that shrinks while the program prints. A Unix pty loses nothing. A ConPTY shrunk early
/// in the output drops the rows above its new screen that it has not rendered yet, a race: on
/// GitHub's Windows runner the case shrunk 5 ms after the cursor answer lost lines 1-22 in 2 of 10
/// runs (CI run 44348226497) and none in 20 (runs 44327828545, 44338573473); every later shrink got
/// 80 of 80 (90 of 90 runs). Whichever way the race goes, the lines that go are the first ones, at
/// most a screen of them (40 rows): the runner holds a smaller size until the output is quiet
/// (`pty::Pending`) for that.
#[test]
fn a_screen_that_shrinks_while_the_program_prints() {
    let _turn = support::hold();
    let cases = [
        (Print::EachLine, 0),
        (Print::EachLine, 5),
        (Print::Paced, 0),
        (Print::Paced, 20),
        (Print::Paced, 100),
    ]
    .map(|(print, ms)| Case {
        print,
        rows: 40,
        answer_after: Duration::ZERO,
        shrink_after: Some(Duration::from_millis(ms)),
    });
    let (runs, report) = probe(&cases);
    if cfg!(unix) {
        assert!(runs.iter().all(whole), "a Unix pty lost lines:\n{report}");
    }
    let head_only = |run: &BTreeSet<u32>| {
        let first = run.first().copied().unwrap_or(LINES + 1);
        first <= 41 && run.len() == (LINES + 1 - first) as usize
    };
    assert!(
        runs.iter().all(head_only),
        "lines other than the first ones, or more than a screen of them, were lost:\n{report}"
    );
}

/// What a terminal emits when it is resized after the output, as the page resizes it when it
/// opens (152x15 in a 1280x720 window) and when the window gets shorter and taller again (the
/// terminal-copy spec). ConPTY keeps no lines above its screen: it repaints from the top row, and
/// after the shrink to 14 rows and the grow back to 15 it starts at the same line, with a blank
/// row at the bottom, where xterm.js would pull a line down from its scrollback (the page sets
/// `windowsPty` for that, apps/web/src/terminal/conpty.ts). A Unix pty emits nothing: the program
/// does not redraw.
#[test]
fn what_a_terminal_emits_when_it_is_resized_after_the_output() {
    let _turn = support::hold();
    let mut term = Term::open(40, steps(Print::EachLine));
    term.until_end(Case {
        print: Print::EachLine,
        rows: 40,
        answer_after: Duration::ZERO,
        shrink_after: None,
    });
    let mut repaints = Vec::new();
    for rows in [15, 14, 15] {
        term.resize(152, rows);
        repaints.push(term.next_for(Duration::from_millis(700)));
    }
    term.close();
    let tops: Vec<Option<u32>> = repaints.iter().map(|r| top_line(r)).collect();
    let repaints: Vec<String> = repaints
        .iter()
        .map(|r| String::from_utf8_lossy(r).into_owned())
        .collect();
    if cfg!(windows) {
        // Run 44317344931: 68, then 69 and 69 (the last line the program printed is "burst over").
        let first = tops[0].expect("a repaint after the first resize");
        assert_eq!(
            tops,
            [Some(first), Some(first + 1), Some(first + 1)],
            "{repaints:?}"
        );
    } else {
        assert!(repaints.iter().all(String::is_empty), "{repaints:?}");
    }
}
