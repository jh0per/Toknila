//! Where the designs a card references are kept, for the card agent's instructions and its
//! permission to read them.

use std::path::PathBuf;

use toknila_core::design::{DesignRef, Region};

use super::Designs;

/// A design a card references, found in the library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub name: String,
    /// The kept file; `None` for a Figma link without an image.
    pub path: Option<PathBuf>,
    pub region: Option<Region>,
}

impl Designs {
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
