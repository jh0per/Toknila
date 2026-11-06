//! What a runner's terminal puts on the wire: binary output frames and a text control frame on
//! `GET /v1/tickets/{ticket}/runner/stream`, and the headers of the scrollback answer.
//!
//! An output frame is a binary WebSocket message: the 26 ASCII characters of the run id, the
//! 8-byte big-endian offset of the payload's first byte in that run's output, then the raw bytes
//! (ANSI escapes kept, for a terminal emulator). The offset counts from the start of the run's
//! log file, so a client that fetched the scrollback up to an offset asks the stream to continue
//! from there. When a run's output is complete the stream sends one text message, a
//! [`RunnerControl`] as JSON.

use serde::{Deserialize, Serialize};
use toknila_core::{ProcessExit, RunId};

/// Characters of a run id in a frame header.
const ID_LEN: usize = 26;
/// Bytes before the payload of an output frame.
pub const FRAME_HEADER: usize = ID_LEN + 8;

/// Header of the scrollback answer: the run it belongs to.
pub const HEADER_RUN: &str = "x-toknila-run";
/// Header of the scrollback answer: offset of its first byte in the run's output.
pub const HEADER_OFFSET: &str = "x-toknila-offset";
/// Header of the scrollback answer: offset after its last byte; the stream continues from here.
pub const HEADER_END: &str = "x-toknila-end";

/// An output frame, decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame<'a> {
    pub run: RunId,
    pub offset: u64,
    pub bytes: &'a [u8],
}

/// The binary message for `bytes` that start at `offset` of `run`'s output.
pub fn encode_frame(run: RunId, offset: u64, bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAME_HEADER + bytes.len());
    out.extend_from_slice(run.to_string().as_bytes());
    out.extend_from_slice(&offset.to_be_bytes());
    out.extend_from_slice(bytes);
    out
}

/// Reads a binary message; `None` if it is not an output frame.
pub fn decode_frame(data: &[u8]) -> Option<Frame<'_>> {
    let (id, rest) = data.split_at_checked(ID_LEN)?;
    let (offset, bytes) = rest.split_at_checked(8)?;
    Some(Frame {
        run: std::str::from_utf8(id).ok()?.parse().ok()?,
        offset: u64::from_be_bytes(offset.try_into().ok()?),
        bytes,
    })
}

/// A text message of the runner stream.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunnerControl {
    /// The run's process ended and every byte it wrote has been sent. `exit` is absent for a
    /// run the daemon found on disk after a restart: how it ended is not known.
    Ended {
        run: RunId,
        #[serde(skip_serializing_if = "Option::is_none")]
        exit: Option<ProcessExit>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> RunId {
        "01JBQ7ZJ0000000000000000R1".parse().expect("id")
    }

    #[test]
    fn a_frame_is_run_id_offset_and_the_raw_bytes() {
        let bytes = b"\x1b[31mred\x1b[0m\r\n";
        let wire = encode_frame(run(), 258, bytes);
        assert_eq!(&wire[..26], b"01JBQ7ZJ0000000000000000R1");
        assert_eq!(&wire[26..34], &[0, 0, 0, 0, 0, 0, 1, 2]);
        assert_eq!(&wire[34..], bytes);
        assert_eq!(
            decode_frame(&wire),
            Some(Frame {
                run: run(),
                offset: 258,
                bytes
            })
        );
    }

    #[test]
    fn a_short_or_foreign_message_is_not_a_frame() {
        assert_eq!(decode_frame(b"too short"), None);
        assert_eq!(decode_frame(&[b'!'; 40]), None);
    }

    #[test]
    fn the_control_frame_is_pinned() {
        let ended = RunnerControl::Ended {
            run: run(),
            exit: Some(ProcessExit {
                code: Some(0),
                signal: None,
            }),
        };
        assert_eq!(
            serde_json::to_string(&ended).expect("json"),
            r#"{"type":"ended","run":"01JBQ7ZJ0000000000000000R1","exit":{"code":0,"signal":null}}"#
        );
        let unknown = RunnerControl::Ended {
            run: run(),
            exit: None,
        };
        assert_eq!(
            serde_json::to_string(&unknown).expect("json"),
            r#"{"type":"ended","run":"01JBQ7ZJ0000000000000000R1"}"#
        );
    }
}
