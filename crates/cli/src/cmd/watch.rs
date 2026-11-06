//! `tok watch`: print every journal entry live, resuming after a reconnect. With `--json` each
//! entry is one line of JSON (the journal record as the API gives it).

use std::io::Write;
use std::time::Duration;

use anyhow::Result;
use toknila_api_types::EventRecord;
use toknila_client::Client;
use toknila_daemon::paths::Paths;

use crate::{daemon_cmd, output};

pub(super) fn run(c: &Client, paths: &Paths, since: Option<i64>, json_out: bool) -> Result<()> {
    let mut last = match since {
        Some(s) => s,
        // Default: only what happens from now on.
        None => c.events(0, 1_000_000)?.last().map(|e| e.seq).unwrap_or(0),
    };
    let mut quiet = false;
    loop {
        // Re-read daemon.json each time: a restarted daemon has a new port and token.
        let url = daemon_cmd::running(paths)?
            .map(|info| Client::new(info.base_url(), info.token).stream_url(last));
        match url.as_deref().map(tungstenite::connect) {
            Some(Ok((mut ws, _))) => {
                quiet = false;
                loop {
                    match ws.read() {
                        Ok(tungstenite::Message::Text(t)) => {
                            let e: EventRecord = serde_json::from_str(t.as_str())?;
                            last = e.seq;
                            if json_out {
                                println!("{}", serde_json::to_string(&e)?);
                            } else {
                                println!("{}", output::event(&e));
                            }
                            // A pipe reader (a script, a test) sees each entry as it happens.
                            std::io::stdout().flush()?;
                        }
                        Ok(tungstenite::Message::Close(_)) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
            }
            _ => {
                if !quiet {
                    eprintln!("(no daemon; waiting to reconnect after #{last})");
                    quiet = true;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
}
