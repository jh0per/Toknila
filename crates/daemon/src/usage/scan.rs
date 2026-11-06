//! Reading transcript files: the only place that touches them. Blocking calls; callers run them
//! off the async workers.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use toknila_core::usage::{Chunk, TranscriptFormat};

/// At most this many bytes are read from one file in one pass; the rest follows next time.
const PASS: u64 = 32 * 1024 * 1024;

/// Folders are searched this deep below a root (project folder, session folder, sub-agents,
/// workflows).
const DEPTH: usize = 6;

/// What one pass over a file found.
#[derive(Debug, Clone)]
pub struct FileRead {
    pub path: PathBuf,
    /// Where this read started (0 when the file shrank and is read again from the top).
    pub from: u64,
    pub chunk: Chunk,
}

impl FileRead {
    /// Where the next read starts.
    pub fn next_offset(&self) -> u64 {
        self.from + self.chunk.consumed as u64
    }
}

/// Every `.jsonl` file below `root`, symbolic links not followed. A missing root has none.
pub fn discover(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(root, DEPTH, &mut found);
    found.sort();
    found
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() && depth > 0 {
            walk(&path, depth - 1, found);
        } else if kind.is_file() && path.extension().is_some_and(|e| e == "jsonl") {
            found.push(path);
        }
    }
}

/// Reads what was appended to `path` since `offset`. `None` when there is nothing new, or the
/// file cannot be read right now (it is tried again next time).
pub fn read_new(format: &dyn TranscriptFormat, path: &Path, offset: u64) -> Option<FileRead> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let from = if len < offset { 0 } else { offset };
    if len == from {
        return None;
    }
    file.seek(SeekFrom::Start(from)).ok()?;
    let mut bytes = Vec::new();
    file.take(PASS.min(len - from))
        .read_to_end(&mut bytes)
        .ok()?;
    let chunk = format.parse(&bytes);
    // Only an unfinished line is new: wait for its end. (A file that shrank is still reported,
    // so its offset goes back to the start.)
    if chunk.consumed == 0 && from == offset {
        return None;
    }
    Some(FileRead {
        path: path.to_owned(),
        from,
        chunk,
    })
}
