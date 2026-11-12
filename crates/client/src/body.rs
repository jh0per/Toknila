//! Reading an answer's body once the daemon may already have closed the connection.

use std::io::Read;

/// The whole body of `resp` as text.
///
/// ureq hands a finished keep-alive connection back to its pool with `set_read_timeout(None)` on
/// the socket, as part of the read that returns 0. When the daemon has closed by then, macOS
/// refuses that with `EINVAL` (`os error 22`) although every byte of the body arrived, and
/// `Response::into_string` turns it into a failure. Such an error after a complete body is
/// ignored; the connection is dropped either way. Complete means: as long as `Content-Length`
/// says, or non-empty when the body is chunked.
pub(crate) fn text(resp: ureq::Response) -> std::io::Result<String> {
    let buf = bytes(resp)?;
    String::from_utf8(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// The whole body of `resp`; see [`text`].
pub(crate) fn bytes(resp: ureq::Response) -> std::io::Result<Vec<u8>> {
    let declared = resp
        .header("content-length")
        .and_then(|v| v.trim().parse::<usize>().ok());
    let mut buf = Vec::new();
    let read = resp.into_reader().read_to_end(&mut buf);
    if let Err(e) = read {
        let complete = match declared {
            Some(n) => buf.len() == n,
            None => !buf.is_empty(),
        };
        if !(is_invalid_argument(&e) && complete) {
            return Err(e);
        }
    }
    Ok(buf)
}

/// Whether `e`, or an error it wraps, says the peer closed the connection: a reset, a broken
/// pipe, an end of stream or the `EINVAL` of a socket option set on a closed socket.
pub(crate) fn lost(e: &(dyn std::error::Error + 'static)) -> bool {
    let mut cur = Some(e);
    while let Some(err) = cur {
        if let Some(io) = err.downcast_ref::<std::io::Error>() {
            use std::io::ErrorKind::{
                BrokenPipe, ConnectionAborted, ConnectionReset, UnexpectedEof,
            };
            return matches!(
                io.kind(),
                ConnectionReset | ConnectionAborted | BrokenPipe | UnexpectedEof
            ) || is_invalid_argument(io);
        }
        cur = err.source();
    }
    false
}

/// Whether `e` is the `EINVAL` of a socket option set on a closed socket.
fn is_invalid_argument(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::InvalidInput || e.raw_os_error() == Some(22)
}
