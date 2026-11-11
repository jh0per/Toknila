//! Embeds the built web page (`apps/web/dist`, from `cargo xtask web`) into the daemon binary.
//! Without a build there, the daemon serves a one-page note saying how to build it.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    migrations(&manifest);
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

/// Collects every `src/store/migrations/*.sql` into one list, ordered by file name, so adding a
/// migration is adding a file and no shared line changes.
fn migrations(manifest: &Path) {
    let dir = manifest.join("src/store/migrations");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("read src/store/migrations")
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            name.ends_with(".sql").then(|| {
                let abs = e.path().canonicalize().unwrap_or_else(|_| e.path());
                (name, abs.to_string_lossy().into_owned())
            })
        })
        .collect();
    files.sort();
    let mut out = String::from(
        "/// Migration files: (file name, SQL), in file-name order.\nstatic FILES: &[(&str, &str)] = &[\n",
    );
    for (name, abs) in &files {
        let _ = writeln!(out, "    ({name:?}, include_str!({abs:?})),");
    }
    out.push_str("];\n");
    let target =
        PathBuf::from(std::env::var("OUT_DIR").expect("set by cargo")).join("migration_files.rs");
    std::fs::write(target, out).expect("write migration_files.rs");
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
