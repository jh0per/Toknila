//! How a card refers to the designs of its project: names turned into references, references
//! checked, and the files the card agent is told and may read.

use std::path::PathBuf;

use toknila_app::planner::DesignPick;
use toknila_core::DesignId;
use toknila_core::design::{self, DesignRef, Region};

use super::{DesignError, Designs};

/// A design a card references, found in the library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub name: String,
    /// The kept file; `None` for a Figma link without an image.
    pub path: Option<PathBuf>,
    pub region: Option<Region>,
}

impl Designs {
    /// The designs as (id, file name) pairs, for chain files.
    pub fn library(&self) -> Vec<(DesignId, String)> {
        self.load()
            .map(|all| all.into_iter().map(|d| (d.id, d.name)).collect())
            .unwrap_or_default()
    }

    /// Turns what a planner named (a file name or an id) into references; a design the project
    /// does not have is refused, with the names it has.
    pub fn resolve(&self, picks: &[DesignPick]) -> Result<Vec<DesignRef>, DesignError> {
        let all = self.load()?;
        let refs = picks
            .iter()
            .map(|p| {
                let found = all
                    .iter()
                    .find(|d| d.id.to_string() == p.design)
                    .or_else(|| all.iter().find(|d| d.name == p.design));
                found
                    .map(|d| DesignRef {
                        design: d.id,
                        region: p.region,
                    })
                    .ok_or_else(|| {
                        let names: Vec<&str> = all.iter().map(|d| d.name.as_str()).collect();
                        DesignError::Invalid(format!(
                            "the project has no design {:?}; its designs: {}",
                            p.design,
                            if names.is_empty() {
                                "none".to_owned()
                            } else {
                                names.join(", ")
                            }
                        ))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        design::check_refs(&refs).map_err(DesignError::Invalid)?;
        Ok(refs)
    }

    /// Refuses references to a design the project does not have.
    pub fn check_refs(&self, refs: &[DesignRef]) -> Result<(), DesignError> {
        let all = self.load()?;
        match refs.iter().find(|r| !all.iter().any(|d| d.id == r.design)) {
            Some(r) => Err(DesignError::Invalid(format!(
                "the project has no design {}",
                r.design
            ))),
            None => Ok(()),
        }
    }

    /// The references that still name a design of the library, in order, with their files. A
    /// design that was deleted is left out.
    pub fn locate(&self, refs: &[DesignRef]) -> Vec<Located> {
        let Ok(all) = self.load() else {
            return Vec::new();
        };
        refs.iter()
            .filter_map(|r| {
                let d = all.iter().find(|d| d.id == r.design)?;
                let path = self.file_of(d).filter(|p| p.is_file());
                Some(Located {
                    name: d.name.clone(),
                    path,
                    region: r.region,
                })
            })
            .collect()
    }
}
