//! The thumbnail of a pdf design: its first page as a png, made by a program of the machine
//! (`sips` on macOS, `pdftoppm` where it is installed). Without one the pdf keeps no thumbnail and
//! the window shows its mark. The renderer runs on a blocking thread, never on a request's.

use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use toknila_api_types::Design;
use toknila_core::DesignId;
use toknila_core::design::{self, DesignKind};

use super::DesignError;

/// How long a renderer may take before its process group is killed.
pub const RENDER_TIMEOUT: Duration = Duration::from_secs(10);

/// The widest side of a thumbnail, in pixels.
const SIDE: &str = "480";

/// Something that draws the first page of a pdf.
pub trait PdfRenderer: Send + Sync + Debug {
    /// Writes the first page of the pdf at `pdf` as a png at `out`; `false` when it could not.
    fn render(&self, pdf: &Path, out: &Path) -> bool;
}

/// The programs of the machine: `sips` (macOS), then `pdftoppm`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemRenderer;

impl PdfRenderer for SystemRenderer {
    fn render(&self, pdf: &Path, out: &Path) -> bool {
        let mut tries: Vec<(&str, Vec<std::ffi::OsString>)> = Vec::new();
        if cfg!(target_os = "macos") {
            tries.push((
                "sips",
                vec![
                    "-s".into(),
                    "format".into(),
                    "png".into(),
                    "-Z".into(),
                    SIDE.into(),
                    pdf.into(),
                    "--out".into(),
                    out.into(),
                ],
            ));
        }
        // pdftoppm adds `.png` to the prefix it is given.
        let prefix = out.with_extension("");
        tries.push((
            "pdftoppm",
            vec![
                "-png".into(),
                "-f".into(),
                "1".into(),
                "-l".into(),
                "1".into(),
                "-singlefile".into(),
                "-scale-to".into(),
                SIDE.into(),
                pdf.into(),
                prefix.into(),
            ],
        ));
        tries
            .into_iter()
            .any(|(program, args)| run_within(program, &args, RENDER_TIMEOUT) && out.is_file())
    }
}

/// No renderer on the machine: a pdf keeps no thumbnail.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoRenderer;

impl PdfRenderer for NoRenderer {
    fn render(&self, _pdf: &Path, _out: &Path) -> bool {
        false
    }
}

/// A renderer that always draws the same bytes (tests).
#[derive(Debug, Clone)]
pub struct FixedRenderer(pub Vec<u8>);

impl PdfRenderer for FixedRenderer {
    fn render(&self, _pdf: &Path, out: &Path) -> bool {
        std::fs::write(out, &self.0).is_ok()
    }
}

/// Runs `program` and waits `limit` at most; a slow one is killed with what it started.
/// `false` when it is missing, fails or is killed.
pub(crate) fn run_within(program: &str, args: &[std::ffi::OsString], limit: Duration) -> bool {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Its own process group, so the timeout ends the program and its helpers.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let Ok(mut child) = cmd.spawn() else {
        return false;
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if started.elapsed() >= limit => {
                crate::git::kill_tree(&mut child);
                let _ = child.wait();
                return false;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return false,
        }
    }
}

impl super::Designs {
    pub(super) fn thumb_path(&self, id: DesignId) -> PathBuf {
        self.dir.join(format!("{id}.thumb.png"))
    }

    /// Makes the first-page thumbnail of a pdf design and records it; a design that is no pdf,
    /// or a machine that cannot render, leaves the design as it is. Takes as long as the
    /// renderer: call it from a blocking task.
    pub fn make_thumb(
        &self,
        id: DesignId,
        renderer: &dyn PdfRenderer,
    ) -> Result<Design, DesignError> {
        let d = self.get(id)?;
        let Some(pdf) = self
            .file_of(&d)
            .filter(|_| d.kind == DesignKind::Pdf && d.has_file)
        else {
            return Ok(d);
        };
        let out = self.thumb_path(id);
        let drawn = renderer.render(&pdf, &out)
            && fs::read(&out)
                .is_ok_and(|b| design::sniff(&b).is_some_and(|k| k.kind == DesignKind::Image));
        if !drawn {
            let _ = fs::remove_file(&out);
            return Ok(d);
        }
        let _w = super::WRITER.lock().unwrap_or_else(|p| p.into_inner());
        let mut all = self.load()?;
        let Some(one) = all.iter_mut().find(|d| d.id == id) else {
            // Removed while it was drawn.
            let _ = fs::remove_file(&out);
            return Err(DesignError::NotFound(id.to_string()));
        };
        one.thumb = true;
        let made = one.clone();
        self.save(&all)?;
        Ok(made)
    }

    /// The picture of a design that has one and its media type: the image itself, or the first
    /// page of a pdf.
    pub fn read_thumb(&self, id: DesignId) -> Result<(Vec<u8>, String), DesignError> {
        let d = self.get(id)?;
        if !d.thumb {
            return Err(DesignError::NotFound(id.to_string()));
        }
        if d.kind == DesignKind::Pdf {
            return Ok((fs::read(self.thumb_path(id))?, "image/png".to_owned()));
        }
        self.read_file(id)
    }

    /// Forgets the thumbnail file of a removed design.
    pub(super) fn remove_thumb(&self, id: DesignId) {
        let _ = fs::remove_file(self.thumb_path(id));
    }
}

/// The page a [`SwitchRenderer`] draws, shared with the test that sets it; `None` draws nothing.
#[cfg(test)]
pub(crate) type Page = std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>>;

/// A renderer a test turns on and off (tests).
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct SwitchRenderer(pub Page);

#[cfg(test)]
impl PdfRenderer for SwitchRenderer {
    fn render(&self, _pdf: &Path, out: &Path) -> bool {
        match &*self.0.lock().unwrap_or_else(|p| p.into_inner()) {
            Some(png) => fs::write(out, png).is_ok(),
            None => false,
        }
    }
}
