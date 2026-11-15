//! Which specs `cargo xtask e2e-web` runs.

/// The specs in `dir` as (name without extension, needs the fake agent): `smoke` first, the rest
/// alphabetical. `helpers.mjs` is a shared module, not a spec. A spec declares the fake agent
/// with `// needs: fake-agent` within its first 15 lines.
pub fn specs(dir: &std::path::Path) -> Result<Vec<(String, bool)>, String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut specs = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
            .path();
        if path.extension().and_then(|e| e.to_str()) != Some("mjs") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if name == "helpers" {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let needs_agent = text
            .lines()
            .take(15)
            .any(|line| line.trim() == "// needs: fake-agent");
        specs.push((name.to_owned(), needs_agent));
    }
    if specs.is_empty() {
        return Err(format!("no e2e spec (*.mjs) found in {}", dir.display()));
    }
    specs.sort_by(|a, b| (a.0 != "smoke", &a.0).cmp(&(b.0 != "smoke", &b.0)));
    Ok(specs)
}
