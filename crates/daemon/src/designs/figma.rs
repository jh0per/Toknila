//! Fetching a frame as a png through Figma's images API
//! (`GET /v1/images/{file_key}?ids={node}&format=png`, the token in `X-Figma-Token`).
//!
//! The request goes through the system's `curl` (TLS without a crate of our own). The token never
//! rides in an argument, where `ps` shows it: it is a header line on curl's standard input. No
//! message made here contains it.

use std::io::Write;
use std::process::{Command, Stdio};

use toknila_core::design::{FigmaRef, MAX_DESIGN_BYTES};

/// Where Figma's API is, unless `TOKNILA_FIGMA_API` says otherwise (a test double).
const DEFAULT_API: &str = "https://api.figma.com";

#[cfg(test)]
pub(crate) static TEST_API: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// The API's base address.
pub fn api_base() -> String {
    #[cfg(test)]
    if let Some(b) = TEST_API.lock().unwrap_or_else(|p| p.into_inner()).clone() {
        return b;
    }
    std::env::var("TOKNILA_FIGMA_API")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_API.to_owned())
}

/// Runs curl for `url`; `token` becomes the `X-Figma-Token` header. The answer's body.
fn curl(url: &str, token: Option<&str>) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new("curl");
    cmd.args([
        "--silent",
        "--show-error",
        "--fail",
        "--max-time",
        "30",
        "--max-filesize",
    ])
    .arg(MAX_DESIGN_BYTES.to_string());
    if token.is_some() {
        // Never follow a redirect with the token.
        cmd.args(["--config", "-"]);
    } else {
        cmd.arg("--location");
    }
    cmd.arg(url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|_| "Could not run curl to reach Figma.".to_owned())?;
    if let Some(mut stdin) = child.stdin.take() {
        if let Some(t) = token {
            let _ = writeln!(stdin, "header = \"X-Figma-Token: {t}\"");
        }
    }
    let out = child
        .wait_with_output()
        .map_err(|_| "Could not read curl's answer.".to_owned())?;
    if out.status.success() {
        return Ok(out.stdout);
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let line = err.lines().next().unwrap_or("").trim().to_owned();
    // A refusal by Figma names the status; the token is never in curl's text, but cut it anyway.
    let line = match token {
        Some(t) => line.replace(t, "…"),
        None => line,
    };
    Err(if line.contains("403") || line.contains("401") {
        "Figma refused the token. Check it in the project's settings.".to_owned()
    } else if line.contains("404") {
        "Figma does not know that file or frame.".to_owned()
    } else {
        format!("Figma could not be reached: {line}")
    })
}

/// The png of a frame.
pub fn fetch_frame(token: &str, frame: &FigmaRef) -> Result<Vec<u8>, String> {
    let ids = frame.node_id.replace(':', "%3A");
    let url = format!(
        "{}/v1/images/{}?ids={ids}&format=png",
        api_base().trim_end_matches('/'),
        frame.file_key
    );
    let body = curl(&url, Some(token))?;
    let json: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| "Figma's answer was not understood.".to_owned())?;
    if let Some(err) = json.get("err").and_then(|e| e.as_str()) {
        return Err(format!("Figma said: {err}"));
    }
    let image = json
        .get("images")
        .and_then(|i| i.get(&frame.node_id))
        .and_then(|u| u.as_str())
        .filter(|u| u.starts_with("https://") || u.starts_with("http://"))
        .ok_or_else(|| "Figma has no image for that frame.".to_owned())?;
    curl(image, None)
}
