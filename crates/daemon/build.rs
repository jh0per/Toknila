//! Embeds the built web page (`apps/web/dist`, from `cargo xtask web`) into the daemon binary.
//! Without a build there, the daemon serves a one-page note saying how to build it.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let dist = manifest.join("../../apps/web/dist");
    println!("cargo:rerun-if-changed={}", dist.display());

    let mut files = Vec::new();
    if dist.join("index.html").is_file() {
        collect(&dist, &dist, &mut files);
    }
    files.sort();
    let mut out = String::from(
        "/// Files of the web page: (path under `/`, bytes, content type).\npub static ASSETS: &[(&str, &[u8], &str)] = &[\n",
    );
    for (rel, abs) in &files {
        let _ = writeln!(
            out,
            "    ({rel:?}, include_bytes!({abs:?}), {:?}),",
            mime(rel)
        );
    }
    out.push_str("];\n");
    let target =
        PathBuf::from(std::env::var("OUT_DIR").expect("set by cargo")).join("web_assets.rs");
    std::fs::write(target, out).expect("write web_assets.rs");
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(root, &p, out);
        } else if let Ok(rel) = p.strip_prefix(root) {
            let rel = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let abs = p.canonicalize().unwrap_or(p).to_string_lossy().into_owned();
            out.push((rel, abs));
        }
    }
}

fn mime(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "json" => "application/json",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}
