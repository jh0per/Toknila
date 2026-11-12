//! What a planner tool answers: a text and, for `show_design`, a second block beside it (the
//! picture itself as MCP image content, or words that there is no file).

use base64_lite::encode;
use serde_json::{Value, json};
use toknila_client::Client;
use toknila_core::design::{is_picture_mime, no_file_words};

/// The answer of one planner tool call.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::cmd) struct Answer {
    /// The first content block, as text.
    pub text: String,
    /// Content blocks after it (an image, or a second text).
    pub more: Vec<Value>,
}

impl From<String> for Answer {
    fn from(text: String) -> Self {
        Self {
            text,
            more: Vec::new(),
        }
    }
}

/// `show_design`: the design's metadata as text, then its picture as image content (png, jpg,
/// webp), or words when there is no file. A pdf answers its metadata and path only.
pub(super) fn show_design(client: &Client, design: &str) -> Result<Answer, String> {
    let found = client.planner_design(design).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&found).map_err(|x| x.to_string())?;
    let more = match (&found.path, found.mime.as_deref()) {
        (None, _) => vec![json!({
            "type": "text",
            "text": no_file_words(&found.name, found.hint.as_deref()),
        })],
        (Some(path), Some(mime)) if is_picture_mime(mime) => {
            let bytes = std::fs::read(path)
                .map_err(|e| format!("cannot read the file of {}: {e}", found.name))?;
            vec![json!({ "type": "image", "data": encode(&bytes), "mimeType": mime })]
        }
        _ => Vec::new(),
    };
    Ok(Answer { text, more })
}

/// Standard base64 with padding (the one encoding MCP image content takes).
mod base64_lite {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    /// `bytes` as base64 text.
    pub fn encode(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let n = chunk
                .iter()
                .enumerate()
                .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(char::from(ABC[((n >> (18 - 6 * i)) & 63) as usize]));
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn encodes_with_padding() {
            assert_eq!(super::encode(b""), "");
            assert_eq!(super::encode(b"f"), "Zg==");
            assert_eq!(super::encode(b"fo"), "Zm8=");
            assert_eq!(super::encode(b"foo"), "Zm9v");
            assert_eq!(super::encode(b"foobar"), "Zm9vYmFy");
        }
    }
}
