//! `cargo xtask guide-shots`: retakes the pictures of the Guide page (`apps/web/src/assets/guide`)
//! from the real window, in light and dark, against a throwaway daemon with example data. Commit
//! the changed pictures with the change that moved a screen.

use super::{app, cargo, ensure_web_bundle, run};

pub(crate) fn guide_shots() -> Result<(), String> {
    let root = app::root()?;
    let web = root.join("apps/web");
    // Before the cargo build: the daemon embeds `dist` when it is compiled, and the pictures
    // must show the window as it is now.
    ensure_web_bundle(&web)?;
    cargo(&["build", "--locked", "--bin", "tok"])?;
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let tok = target
        .join("debug")
        .join(format!("tok{}", std::env::consts::EXE_SUFFIX))
        .to_string_lossy()
        .into_owned();
    run("node", &["scripts/guide-shots.mjs", &tok], &web)?;
    eprintln!("xtask guide-shots: pictures written to apps/web/src/assets/guide");
    Ok(())
}
