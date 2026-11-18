//! TOKNILA-162 probe, layer "terminal": which of 80 numbered lines a program prints reach the
//! reader of its terminal, with no daemon code in between. The program prints them as fast as it
//! can (one write of all 80, or 80 writes with no pause), at the runner's screen size and at a
//! taller one, with the cursor query answered at once or late (a busy daemon).
//!
//! On Unix a pty hands the reader the program's bytes as they are, so every case gets all 80: the
//! control that the counting works. Windows' ConPTY is not a byte pipe: it keeps a screen, renders
//! it now and then and emits VT for what it rendered, so a line that scrolled off the top of its
//! screen between two renders never reaches the reader.

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
}

#[derive(Debug, Clone, Copy)]
struct Case {
    print: Print,
    rows: u16,
    /// How long after the cursor query the answer is written (ConPTY asks once, at its start).
    answer_after: Duration,
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
    asked: Option<Instant>,
    answered: bool,
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
            asked: None,
            answered: false,
        }
    }

    /// Takes what arrives within 10 ms, and answers ConPTY's cursor query `answer_after` it came.
    fn pump(&mut self, answer_after: Duration) {
        if let Ok(chunk) = self.rx.recv_timeout(Duration::from_millis(10)) {
            self.got.extend_from_slice(&chunk);
        }
        if self.asked.is_none() && self.got.windows(4).any(|w| w == b"\x1b[6n") {
            self.asked = Some(Instant::now());
        }
        if !self.answered && self.asked.is_some_and(|t| t.elapsed() >= answer_after) {
            self.answered = true;
            self.writer.write_all(b"\x1b[1;1R").expect("answer");
        }
    }

    /// Reads until the program's last line has arrived (or [`WAIT`] has passed).
    fn until_end(&mut self, answer_after: Duration) {
        let until = Instant::now() + WAIT;
        while Instant::now() < until && !plain(&self.got).contains(END) {
            self.pump(answer_after);
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
    term.until_end(case.answer_after);
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
    format!(
        "{:?} rows={} answer_after={}ms: got [{}] of {LINES}; worst missed [{}]",
        case.print,
        case.rows,
        case.answer_after.as_millis(),
        counts.join(" "),
        ranges(&missing)
    )
}

#[test]
fn the_escape_filter_and_the_counter_find_whole_numbered_lines() {
    let bytes = b"\x1b[?25l\x1b[2J\x1b[Hscroll line 1\x1b[K\r\nscroll \x1b[mline 2\r\n\x1b]0;t\x07scroll line 10\r\nscroll line 81";
    // A line split by a sequence counts; 81 is past the program's lines.
    assert_eq!(numbers(&plain(bytes)), BTreeSet::from([1, 2, 10]));
    assert_eq!(ranges(&[1, 2, 3, 5, 70, 71]), "1-3, 5, 70-71");
}

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
    ];
    let mut lines = Vec::new();
    let mut lost = false;
    for (print, rows, answer_after) in cases {
        let case = Case {
            print,
            rows,
            answer_after,
        };
        let runs: Vec<BTreeSet<u32>> = (0..REPS).map(|_| run(case)).collect();
        lost |= runs.iter().any(|r| r.len() < LINES as usize);
        let line = report(case, &runs);
        eprintln!("TOKNILA-162 probe: {line}");
        lines.push(line);
    }
    // A pty is a byte pipe: nothing is ever lost (the control).
    if cfg!(unix) {
        assert!(!lost, "a Unix pty lost lines:\n{}", lines.join("\n"));
    }
}

/// What a terminal emits when it is resized after the output, as the page resizes it when it
/// opens (152x15 in a 1280x720 window) and when the window gets shorter and taller again (the
/// terminal-copy spec).
#[test]
fn what_a_terminal_emits_when_it_is_resized_after_the_output() {
    let _turn = support::hold();
    let mut term = Term::open(40, steps(Print::EachLine));
    term.until_end(Duration::ZERO);
    eprintln!(
        "TOKNILA-162 resize: the output at 160x40: {:?}",
        String::from_utf8_lossy(&term.got)
    );
    for rows in [15, 14, 15] {
        term.master
            .resize(PtySize {
                rows,
                cols: 152,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("resize");
        let after = term.next_for(Duration::from_millis(700));
        eprintln!(
            "TOKNILA-162 resize: after the resize to 152x{rows}: {:?}",
            String::from_utf8_lossy(&after)
        );
    }
    term.close();
}
