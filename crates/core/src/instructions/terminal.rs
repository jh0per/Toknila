//! Bounds of the terminal a person's browser may ask for. The agent is a full-screen terminal
//! application, so the size must follow the panel, but never to a size that breaks it.

/// Columns and rows of a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

/// The size a new terminal starts with, before any client has fitted it.
pub const DEFAULT: Size = Size {
    cols: 160,
    rows: 40,
};
/// Smallest and largest accepted sizes.
pub const MIN: Size = Size { cols: 20, rows: 5 };
pub const MAX: Size = Size {
    cols: 500,
    rows: 200,
};

/// The requested size brought into bounds.
pub fn clamp(cols: u32, rows: u32) -> Size {
    let fit = |v: u32, lo: u16, hi: u16| u16::try_from(v).unwrap_or(u16::MAX).clamp(lo, hi);
    Size {
        cols: fit(cols, MIN.cols, MAX.cols),
        rows: fit(rows, MIN.rows, MAX.rows),
    }
}
