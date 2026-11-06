//! `tok logs`: print a ticket's terminal output, and with `--follow` keep printing it live.

use std::io::Write;

use anyhow::{Context, Result, bail};
use toknila_api_types::runner::{RunnerControl, decode_frame};
use toknila_client::Client;

/// Prints the scrollback of the ticket's newest run (raw, ANSI kept). With `follow`, then
/// streams what the agent writes next, until its run ends.
pub(super) fn run(c: &Client, ticket: &str, follow: bool) -> Result<()> {
    let scrollback = c.runner_scrollback(ticket)?;
    let mut out = std::io::stdout().lock();
    out.write_all(&scrollback.bytes)?;
    out.flush()?;
    if !follow {
        return Ok(());
    }
    let url = c.runner_stream_url(ticket, Some((scrollback.run, scrollback.end)));
    let (mut ws, _) = tungstenite::connect(url).context("cannot open the output stream")?;
    let mut run = scrollback.run;
    loop {
        match ws.read().context("the output stream broke")? {
            tungstenite::Message::Binary(data) => {
                let frame = decode_frame(&data).context("malformed output frame")?;
                run = frame.run;
                out.write_all(frame.bytes)?;
                out.flush()?;
            }
            tungstenite::Message::Text(text) => {
                let RunnerControl::Ended { run: ended, exit } =
                    serde_json::from_str(text.as_str()).context("malformed control frame")?;
                if ended == run {
                    if let Some(exit) = exit {
                        eprintln!("(the agent exited: {exit:?})");
                    }
                    return Ok(());
                }
            }
            tungstenite::Message::Close(_) => bail!("the daemon closed the output stream"),
            _ => {}
        }
    }
}
