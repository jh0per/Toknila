//! A project's designs on disk: `<home>/designs/<project id>/` holds `index.json` and one file
//! per design (`<design id>.<ext>`); the project's Figma token is a file of its own under
//! `<home>/secrets/`, readable by its owner only. Nothing here is in the repository or in the
//! database, and nothing here writes the token to a log.

pub mod figma;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use toknila_api_types::{Design, PlannerDesign};
use toknila_core::design::{self, DesignKind, FigmaRef, MAX_DESIGN_BYTES};
use toknila_core::{DesignId, ProjectId, Timestamp};

/// One writer at a time: the index is read, changed and written back.
static WRITER: Mutex<()> = Mutex::new(());

/// Why a design change was refused.
#[derive(Debug, thiserror::Error)]
pub enum DesignError {
    /// The input is wrong; the text says how.
    #[error("{0}")]
    Invalid(String),
    /// No such design.
    #[error("design {0} not found")]
    NotFound(String),
    /// The disk refused.
    #[error("designs: {0}")]
    Io(#[from] io::Error),
}

/// The hint of a Figma link that has no image yet.
const EXPORT_HINT: &str = "Export the frame from Figma as a png and drop it here, or add a Figma token in the project's settings so Toknila can fetch it.";
const FETCH_HINT: &str = "A Figma token is saved; fetch the frame to keep its image.";

/// The designs of one project.
#[derive(Debug, Clone)]
pub struct Designs {
    dir: PathBuf,
    secret: PathBuf,
}

impl Designs {
    /// The designs of `project` under the data `home`.
    pub fn new(home: &Path, project: ProjectId) -> Self {
        Self {
            dir: home.join("designs").join(project.to_string()),
            secret: home.join("secrets").join(format!("figma-{project}.token")),
        }
    }

    fn index(&self) -> PathBuf {
        self.dir.join("index.json")
    }

    fn load(&self) -> Result<Vec<Design>, DesignError> {
        match fs::read(self.index()) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| DesignError::Io(io::Error::new(io::ErrorKind::InvalidData, e))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.into()),
        }
    }

    fn save(&self, all: &[Design]) -> Result<(), DesignError> {
        fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join("index.json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(all).unwrap_or_default())?;
        fs::rename(&tmp, self.index())?;
        Ok(())
    }

    fn file_of(&self, d: &Design) -> Option<PathBuf> {
        let ext = ext_of(d.mime.as_deref()?)?;
        Some(self.dir.join(format!("{}.{ext}", d.id)))
    }

    /// The designs as the planner sees them: metadata and the path of each kept file.
    pub fn for_planner(&self) -> Result<Vec<PlannerDesign>, DesignError> {
        Ok(self
            .list()?
            .into_iter()
            .map(|d| PlannerDesign {
                path: d
                    .has_file
                    .then(|| self.file_of(&d))
                    .flatten()
                    .map(|p| p.display().to_string()),
                id: d.id,
                name: d.name,
                kind: d.kind,
                mime: d.mime,
                size: d.size,
                hint: d.hint,
            })
            .collect())
    }

    /// The paths of the kept design files, which a planner may read.
    pub fn file_paths(&self) -> Vec<PathBuf> {
        self.for_planner()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|d| d.path.map(PathBuf::from))
            .collect()
    }

    /// One design for the planner, by id or by name (any letter case).
    pub fn find_for_planner(&self, reference: &str) -> Result<PlannerDesign, DesignError> {
        let all = self.for_planner()?;
        let reference = reference.trim();
        all.into_iter()
            .find(|d| d.id.to_string() == reference || d.name.eq_ignore_ascii_case(reference))
            .ok_or_else(|| DesignError::NotFound(reference.to_owned()))
    }

    /// Every design, oldest first, with the hint a link without an image shows.
    pub fn list(&self) -> Result<Vec<Design>, DesignError> {
        let has_token = self.has_token();
        let mut all = self.load()?;
        for d in &mut all {
            if d.kind == DesignKind::Figma && !d.has_file && d.hint.is_none() {
                d.hint = Some(if has_token { FETCH_HINT } else { EXPORT_HINT }.to_owned());
            }
        }
        Ok(all)
    }

    /// The file names of the designs, for a chain export: a Figma link is named by its name,
    /// never by its address.
    pub fn names(&self) -> Vec<String> {
        self.load()
            .map(|all| all.into_iter().map(|d| d.name).collect())
            .unwrap_or_default()
    }

    /// Keeps an uploaded file. The kind comes from the bytes, not from the name.
    pub fn add_file(&self, name: &str, bytes: &[u8], at: Timestamp) -> Result<Design, DesignError> {
        let name = design::clean_name(name).map_err(DesignError::Invalid)?;
        if bytes.len() > MAX_DESIGN_BYTES {
            return Err(DesignError::Invalid(format!(
                "A design file is at most {} MB.",
                MAX_DESIGN_BYTES / (1024 * 1024)
            )));
        }
        let kind = design::sniff(bytes).ok_or_else(|| {
            DesignError::Invalid("Only png, jpg, webp and pdf files can be designs.".to_owned())
        })?;
        let _w = WRITER.lock().unwrap_or_else(|p| p.into_inner());
        let mut all = self.load()?;
        let d = Design {
            id: DesignId::new(),
            name,
            kind: kind.kind,
            mime: Some(kind.mime.to_owned()),
            size: bytes.len() as u64,
            created_at: at,
            figma: None,
            has_file: true,
            thumb: kind.kind == DesignKind::Image,
            hint: None,
        };
        fs::create_dir_all(&self.dir)?;
        if let Some(path) = self.file_of(&d) {
            fs::write(path, bytes)?;
        }
        all.push(d.clone());
        self.save(&all)?;
        Ok(d)
    }

    /// Keeps a Figma link, without its image.
    pub fn add_figma(
        &self,
        name: &str,
        frame: FigmaRef,
        at: Timestamp,
    ) -> Result<Design, DesignError> {
        let name = design::clean_name(name).map_err(DesignError::Invalid)?;
        let _w = WRITER.lock().unwrap_or_else(|p| p.into_inner());
        let mut all = self.load()?;
        let d = Design {
            id: DesignId::new(),
            name,
            kind: DesignKind::Figma,
            mime: None,
            size: 0,
            created_at: at,
            figma: Some(frame),
            has_file: false,
            thumb: false,
            hint: None,
        };
        all.push(d.clone());
        self.save(&all)?;
        Ok(d)
    }

    /// Keeps the fetched image of a Figma design (`Ok(None)` with `Err` text leaves the link as
    /// it is and records why in its hint).
    pub fn attach_image(
        &self,
        id: DesignId,
        fetched: Result<&[u8], String>,
    ) -> Result<Design, DesignError> {
        let _w = WRITER.lock().unwrap_or_else(|p| p.into_inner());
        let mut all = self.load()?;
        let d = all
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or_else(|| DesignError::NotFound(id.to_string()))?;
        match fetched {
            Ok(bytes) => {
                let kind = design::sniff(bytes)
                    .filter(|k| k.kind == DesignKind::Image)
                    .ok_or_else(|| {
                        DesignError::Invalid("Figma sent something that is not an image.".into())
                    })?;
                d.mime = Some(kind.mime.to_owned());
                d.size = bytes.len() as u64;
                d.has_file = true;
                d.thumb = true;
                d.hint = None;
                fs::create_dir_all(&self.dir)?;
                if let Some(path) = self.file_of(d) {
                    fs::write(path, bytes)?;
                }
            }
            Err(why) => d.hint = Some(why),
        }
        let out = d.clone();
        self.save(&all)?;
        Ok(out)
    }

    /// One design.
    pub fn get(&self, id: DesignId) -> Result<Design, DesignError> {
        self.list()?
            .into_iter()
            .find(|d| d.id == id)
            .ok_or_else(|| DesignError::NotFound(id.to_string()))
    }

    /// The kept file of a design and its media type.
    pub fn read_file(&self, id: DesignId) -> Result<(Vec<u8>, String), DesignError> {
        let d = self.get(id)?;
        let (Some(path), Some(mime)) = (self.file_of(&d), d.mime) else {
            return Err(DesignError::NotFound(id.to_string()));
        };
        Ok((fs::read(path)?, mime))
    }

    /// Renames a design.
    pub fn rename(&self, id: DesignId, name: &str) -> Result<Design, DesignError> {
        let name = design::clean_name(name).map_err(DesignError::Invalid)?;
        let _w = WRITER.lock().unwrap_or_else(|p| p.into_inner());
        let mut all = self.load()?;
        let d = all
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or_else(|| DesignError::NotFound(id.to_string()))?;
        d.name = name;
        let out = d.clone();
        self.save(&all)?;
        Ok(out)
    }

    /// Removes a design and its file.
    pub fn remove(&self, id: DesignId) -> Result<(), DesignError> {
        let _w = WRITER.lock().unwrap_or_else(|p| p.into_inner());
        let mut all = self.load()?;
        let at = all
            .iter()
            .position(|d| d.id == id)
            .ok_or_else(|| DesignError::NotFound(id.to_string()))?;
        let d = all.remove(at);
        if let Some(path) = self.file_of(&d) {
            let _ = fs::remove_file(path);
        }
        self.save(&all)
    }

    /// Removes everything of the project: its designs and its token.
    pub fn delete_all(&self) {
        let _ = fs::remove_dir_all(&self.dir);
        let _ = fs::remove_file(&self.secret);
    }

    /// Whether a Figma token is saved.
    pub fn has_token(&self) -> bool {
        self.secret.is_file()
    }

    /// The saved Figma token.
    pub fn token(&self) -> Option<String> {
        fs::read_to_string(&self.secret)
            .ok()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
    }

    /// Saves the token as a file only its owner can read.
    pub fn set_token(&self, token: &str) -> Result<(), DesignError> {
        let token = design::clean_token(token).map_err(DesignError::Invalid)?;
        if let Some(parent) = self.secret.parent() {
            fs::create_dir_all(parent)?;
        }
        write_private(&self.secret, token.as_bytes())?;
        Ok(())
    }

    /// Forgets the token.
    pub fn clear_token(&self) -> Result<(), DesignError> {
        match fs::remove_file(&self.secret) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }
}

fn ext_of(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "application/pdf" => Some("pdf"),
        _ => None,
    }
}

/// Writes a file that only its owner may read (mode 0600 on Unix; elsewhere the profile folder's
/// own access rules apply).
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    f.write_all(bytes)
}

#[cfg(test)]
pub(crate) mod tests;
