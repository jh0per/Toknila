//! Designs of a project: which files count, what a Figma frame link names, and how a design is
//! called. Pure rules; the daemon keeps the files.

use serde::{Deserialize, Serialize};

use crate::DesignId;

/// Most bytes one design file may have.
pub const MAX_DESIGN_BYTES: usize = 25 * 1024 * 1024;

/// What a design is.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignKind {
    /// A png, jpg or webp file.
    Image,
    /// A pdf, kept as one file.
    Pdf,
    /// A link to a Figma frame.
    Figma,
}

/// What the first bytes of a file say it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sniffed {
    pub kind: DesignKind,
    pub mime: &'static str,
    pub ext: &'static str,
}

/// Tells png, jpg, webp and pdf from the file's own bytes (never from its name); `None` for
/// anything else.
pub fn sniff(bytes: &[u8]) -> Option<Sniffed> {
    let image = |mime, ext| Sniffed {
        kind: DesignKind::Image,
        mime,
        ext,
    };
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some(image("image/png", "png"))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(image("image/jpeg", "jpg"))
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(image("image/webp", "webp"))
    } else if bytes.starts_with(b"%PDF-") {
        Some(Sniffed {
            kind: DesignKind::Pdf,
            mime: "application/pdf",
            ext: "pdf",
        })
    } else {
        None
    }
}

/// The frame a Figma link points at.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigmaRef {
    /// The link as the person gave it.
    pub url: String,
    pub file_key: String,
    /// The frame's node id, `12:34`.
    pub node_id: String,
}

/// Reads a Figma frame link (`figma.com/design/<key>/<name>?node-id=12-34`; `file`, `proto` and
/// `board` links too). A link without a frame is refused: the images API needs one.
pub fn parse_figma(link: &str) -> Result<FigmaRef, String> {
    let link = link.trim();
    let rest = link
        .strip_prefix("https://")
        .or_else(|| link.strip_prefix("http://"))
        .ok_or("A Figma link starts with https://www.figma.com/")?;
    let (host, path_query) = rest.split_once('/').unwrap_or((rest, ""));
    if host != "figma.com" && !host.ends_with(".figma.com") {
        return Err("That is not a figma.com link.".to_owned());
    }
    let (path, query) = path_query.split_once('?').unwrap_or((path_query, ""));
    let path = path.split('#').next().unwrap_or("");
    let mut parts = path.split('/').filter(|p| !p.is_empty());
    if !matches!(
        parts.next(),
        Some("design" | "file" | "proto" | "board" | "make")
    ) {
        return Err("Give the link of a frame: Figma's Copy link to selection.".to_owned());
    }
    let mut key = parts.next().unwrap_or("").to_owned();
    // A branch of the file has its own key.
    let rest: Vec<&str> = parts.collect();
    if let Some(i) = rest.iter().position(|p| *p == "branch")
        && let Some(b) = rest.get(i + 1)
    {
        (*b).clone_into(&mut key);
    }
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("The link has no file key.".to_owned());
    }
    let node = query
        .split('#')
        .next()
        .unwrap_or("")
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == "node-id")
        .map(|(_, v)| v.replace("%3A", ":").replace('-', ":"))
        .unwrap_or_default();
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let ok = node
        .split([':', ';'])
        .all(|n| digits(n.trim_start_matches('I')));
    if node.is_empty() || !ok {
        return Err(
            "The link names no frame (node-id). Select the frame and use Copy link to selection."
                .to_owned(),
        );
    }
    Ok(FigmaRef {
        url: link.to_owned(),
        file_key: key,
        node_id: node,
    })
}

/// A design's name: one line, no path, at most 120 characters. The final path segment of what was
/// given, so a dropped `C:\x\y.png` is `y.png`.
pub fn clean_name(raw: &str) -> Result<String, String> {
    let last = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let name: String = last
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_owned();
    if name.is_empty() || name == "." || name == ".." {
        return Err("A design needs a name.".to_owned());
    }
    Ok(name.chars().take(120).collect())
}

/// A rectangle of a design image, in whole percent of the image's width and height.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub x: u8,
    pub y: u8,
    pub w: u8,
    pub h: u8,
}

impl Region {
    /// Whether the rectangle is not empty and lies inside the image.
    pub fn check(&self) -> Result<(), String> {
        let (x, y, w, h) = (
            u16::from(self.x),
            u16::from(self.y),
            u16::from(self.w),
            u16::from(self.h),
        );
        if w == 0 || h == 0 {
            return Err("A region needs a width and a height above 0.".to_owned());
        }
        if x + w > 100 || y + h > 100 {
            return Err(
                "A region lies inside the image: x + w and y + h are at most 100 (percent)."
                    .to_owned(),
            );
        }
        Ok(())
    }
}

/// A card's reference to a design of its project, with the part of the image that matters.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignRef {
    pub design: DesignId,
    /// `None`: the whole image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
}

/// Checks a card's list of references: every region is valid and no reference repeats.
pub fn check_refs(refs: &[DesignRef]) -> Result<(), String> {
    for (i, r) in refs.iter().enumerate() {
        if let Some(region) = &r.region {
            region.check()?;
        }
        if refs[..i].contains(r) {
            return Err("The same design with the same region is listed twice.".to_owned());
        }
    }
    Ok(())
}

/// A personal access token as Figma issues them: printable characters without spaces or quotes.
pub fn clean_token(raw: &str) -> Result<String, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Err("The token is empty.".to_owned());
    }
    if t.len() > 200
        || !t
            .chars()
            .all(|c| c.is_ascii_graphic() && c != '"' && c != '\\')
    {
        return Err("That does not look like a Figma token.".to_owned());
    }
    Ok(t.to_owned())
}

#[cfg(test)]
mod tests;
