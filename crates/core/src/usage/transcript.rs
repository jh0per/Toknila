//! Reading an agent's transcript. A format turns the bytes of a transcript, from an offset on,
//! into model calls. Claude Code only for now; another agent kind adds another format.

use std::collections::HashMap;

use serde::Deserialize;

use super::{Record, TokenCounts, parse_rfc3339};

/// What one read of a transcript found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chunk {
    /// Model calls, each once, in the order they first appeared.
    pub records: Vec<Record>,
    /// Bytes read up to the end of the last complete line; the next read starts here.
    pub consumed: usize,
    /// The first working directory the transcript names.
    pub cwd: Option<String>,
}

/// An agent kind's transcript format.
pub trait TranscriptFormat: Send + Sync {
    /// The agent kind, for example `claude-code`.
    fn kind(&self) -> &'static str;
    /// Reads complete lines of `bytes`; an unfinished last line is left for the next read.
    fn parse(&self, bytes: &[u8]) -> Chunk;
}

/// Claude Code's JSONL transcripts: an `assistant` line carries `message.usage`. One call is
/// written as several lines (one per content block, and while streaming) with the same message
/// id and request id; it counts once, with the largest number of each field.
#[derive(Debug, Clone, Copy)]
pub struct ClaudeCode;

#[derive(Deserialize)]
struct Line {
    uuid: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    timestamp: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "isApiErrorMessage")]
    api_error: Option<bool>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
}

#[derive(Deserialize, Default)]
struct Usage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

impl Line {
    fn record(self) -> Option<Record> {
        if self.api_error == Some(true) {
            return None;
        }
        let message = self.message?;
        let model = message
            .model
            .filter(|m| !m.is_empty() && !m.starts_with('<'))?;
        let u = message.usage?;
        let tokens = TokenCounts {
            input: u.input_tokens,
            cache_write: u.cache_creation_input_tokens,
            cache_read: u.cache_read_input_tokens,
            output: u.output_tokens,
        };
        if tokens.processed() == 0 {
            return None;
        }
        let key = match (message.id, self.request_id, self.uuid) {
            (Some(id), Some(req), _) => format!("{id}/{req}"),
            (Some(id), None, _) => id,
            (None, _, Some(uuid)) => uuid,
            _ => return None,
        };
        let at = self.timestamp.as_deref().and_then(parse_rfc3339)?;
        Some(Record {
            key,
            at,
            model,
            tokens,
        })
    }
}

fn larger(a: &mut TokenCounts, b: &TokenCounts) {
    a.input = a.input.max(b.input);
    a.cache_write = a.cache_write.max(b.cache_write);
    a.cache_read = a.cache_read.max(b.cache_read);
    a.output = a.output.max(b.output);
}

impl TranscriptFormat for ClaudeCode {
    fn kind(&self) -> &'static str {
        "claude-code"
    }

    fn parse(&self, bytes: &[u8]) -> Chunk {
        let end = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        let mut chunk = Chunk {
            consumed: end,
            ..Chunk::default()
        };
        let mut at: HashMap<String, usize> = HashMap::new();
        for raw in bytes[..end].split(|b| *b == b'\n') {
            // Only assistant lines carry usage; skip the rest without parsing them.
            let usage_line = raw.windows(7).any(|w| w == b"\"usage\"");
            if raw.is_empty() || (!usage_line && chunk.cwd.is_some()) {
                continue;
            }
            let Ok(line) = serde_json::from_slice::<Line>(raw) else {
                continue;
            };
            if chunk.cwd.is_none() {
                chunk.cwd = line.cwd.clone().filter(|c| !c.is_empty());
            }
            let Some(record) = line.record() else {
                continue;
            };
            match at.get(&record.key) {
                Some(i) => larger(&mut chunk.records[*i].tokens, &record.tokens),
                None => {
                    at.insert(record.key.clone(), chunk.records.len());
                    chunk.records.push(record);
                }
            }
        }
        chunk
    }
}

/// The session a Claude Code transcript file belongs to: `<root>/<project folder>/<session>.jsonl`,
/// or a file under the folder `<root>/<project folder>/<session>/` (sub-agents, workflows).
pub fn session_of(path: &str, roots: &[String]) -> Option<String> {
    let path_parts = parts(path);
    roots.iter().find_map(|root| {
        let rest = path_parts.strip_prefix(parts(root).as_slice())?;
        let [_project, session, ..] = rest else {
            return None;
        };
        Some(session.strip_suffix(".jsonl").unwrap_or(session).to_owned())
    })
}

/// The non-empty parts of a path; either slash separates.
fn parts(p: &str) -> Vec<String> {
    p.replace('\\', "/")
        .split('/')
        .filter(|c| !c.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Whether a transcript path a hook reported may be read: an absolute `.jsonl` path inside one
/// of the agent's own folders (`roots`), with no `.` or `..` part. Either slash is a separator.
pub fn accepted_transcript(path: &str, roots: &[String]) -> bool {
    let absolute = path.starts_with('/') || path.starts_with('\\') || path.get(1..2) == Some(":");
    let path_parts = parts(path);
    let clean = path_parts.iter().all(|c| c != ".." && c != ".");
    absolute
        && clean
        && path.ends_with(".jsonl")
        && roots.iter().any(|root| {
            let r = parts(root);
            !r.is_empty()
                && path_parts.len() > r.len()
                && path_parts.iter().zip(&r).all(|(a, b)| a == b)
        })
}
