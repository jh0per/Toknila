//! Playwright's Chromium for `cargo xtask e2e-web`: asks Playwright where it lives and whether
//! it is there, so a browser another Playwright version pruned is installed again.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The install locations of Chromium builds in `playwright install --dry-run` output.
pub fn chromium_locations(dry_run: &str) -> Vec<PathBuf> {
    dry_run
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Install location:"))
        .map(|rest| PathBuf::from(rest.trim()))
        .filter(|path| {
            path.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("chromium"))
        })
        .collect()
}

/// Whether some Chromium build `dry_run` names is missing according to `exists`.
pub fn needs_install(dry_run: &str, exists: impl Fn(&Path) -> bool) -> bool {
    chromium_locations(dry_run).iter().any(|p| !exists(p))
}

/// Installs Chromium when Playwright's copy is missing; prints one line when it does.
pub fn ensure_chromium(web: &Path) -> Result<(), String> {
    let npx = if cfg!(windows) { "npx.cmd" } else { "npx" };
    let out = Command::new(npx)
        .args(["playwright", "install", "--dry-run", "chromium"])
        .current_dir(web)
        .output()
        .map_err(|e| format!("cannot run {npx}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "playwright install --dry-run failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    if !needs_install(&text, |p| p.exists()) {
        return Ok(());
    }
    eprintln!("xtask e2e-web: installing Playwright's Chromium (it was missing)");
    crate::run(npx, &install_args(), web)
}

/// Arguments of the install command (`--with-deps` on Linux).
pub fn install_args() -> Vec<&'static str> {
    let mut args = vec!["playwright", "install", "chromium"];
    if cfg!(target_os = "linux") {
        args.push("--with-deps");
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    const DRY: &str = "Chrome for Testing 153.0.8010.12 (playwright chromium v1243)
  Install location:    /c/ms-playwright/chromium-1243
  Download url:        https://x/chrome.zip

FFmpeg (playwright ffmpeg v1011)
  Install location:    /c/ms-playwright/ffmpeg-1011
  Download url:        https://x/ffmpeg.zip

Chrome Headless Shell 153.0.8010.12 (playwright chromium-headless-shell v1243)
  Install location:    /c/ms-playwright/chromium_headless_shell-1243
  Download url:        https://x/shell.zip
";

    #[test]
    fn reads_chromium_locations_and_skips_ffmpeg() {
        assert_eq!(
            chromium_locations(DRY),
            vec![
                PathBuf::from("/c/ms-playwright/chromium-1243"),
                PathBuf::from("/c/ms-playwright/chromium_headless_shell-1243"),
            ]
        );
    }

    #[test]
    fn installs_when_a_browser_folder_is_missing() {
        assert!(needs_install(DRY, |_| false));
        assert!(needs_install(DRY, |p| !p.ends_with("chromium_headless_shell-1243")));
    }

    #[test]
    fn leaves_a_present_browser_alone() {
        assert!(!needs_install(DRY, |_| true));
    }

    #[test]
    fn output_without_a_chromium_entry_does_not_install() {
        assert!(!needs_install("nothing here", |_| false));
    }

    #[test]
    fn checks_real_paths() {
        let dir =
            std::env::temp_dir().join(format!("toknila-xtask-browser-{}", std::process::id()));
        let have = dir.join("chromium-1");
        std::fs::create_dir_all(&have).expect("make dir");
        let text = |p: &Path| {
            format!(
                "Chrome (playwright chromium v1)\n  Install location:    {}\n",
                p.display()
            )
        };
        assert!(!needs_install(&text(&have), |p| p.exists()));
        assert!(needs_install(&text(&dir.join("chromium-2")), |p| p.exists()));
        std::fs::remove_dir_all(&dir).expect("clean up");
    }
}
