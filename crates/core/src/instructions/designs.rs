//! The designs of a card in the instructions: the files the agent may read, and the "design
//! first" rule for a card whose design a person approves before any code.

use super::RunBrief;

/// A design a card references, as the instructions name it: the file the agent may read and the
/// part of the image that matters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefDesign {
    pub name: String,
    /// The design's kept file; `None` for a Figma link without an image.
    pub path: Option<String>,
    pub region: Option<crate::design::Region>,
}

/// The section that names the designs of the card, with the files the agent may read; empty
/// when the card has none.
pub(super) fn designs_section(brief: &RunBrief<'_>) -> String {
    if brief.designs.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n## Designs\n\n\
         The story is about these designs. Read the files (an image or a pdf) before you build \
         what they show; you may read these files and no others of the library. A region is in \
         percent of the image, from its top left corner.\n\n",
    );
    for d in brief.designs {
        out.push_str(&format!("- {}: ", d.name));
        match &d.path {
            Some(p) => out.push_str(&format!("`{p}`")),
            None => out.push_str("no image is kept for it (a Figma link)"),
        }
        if let Some(r) = d.region {
            out.push_str(&format!(
                "; region: x {}%, y {}%, width {}%, height {}%",
                r.x, r.y, r.w, r.h
            ));
        }
        out.push('\n');
    }
    out
}

/// The section of the instructions file for a card that must show a design first; empty
/// otherwise.
pub(super) fn design_section(brief: &RunBrief<'_>) -> String {
    if !brief.design_first {
        return String::new();
    }
    "\n## Design first\n\n\
     The person asked to see and approve a design before you write any code. Before you touch \
     the code: call `open_thread` with kind `design` (kind `flow` or `architecture` when that \
     fits the story better), then post a first version with `thread_version` that the person can \
     judge: a Mermaid diagram, or the screen described as its states and layout. Then stop and \
     wait: the person's approval or request for changes arrives as an instruction. Do not ask \
     for it with `ask_human`, and do not open a second thread for the same design; answer a \
     request with a new version in the same thread. Write code only after the person approved a \
     version. `finished` is refused until one is approved.\n"
        .to_owned()
}
