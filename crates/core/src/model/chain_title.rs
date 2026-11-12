//! An epic's title: the words a person reads, kept apart from the name that is the identifier of
//! folders and branches. Both directions of the derivation live here.

use super::Chain;

impl Chain {
    /// The words a person reads for the chain: its title, or the name made readable.
    pub fn display_title(&self) -> String {
        match self.title.as_deref().map(str::trim) {
            Some(t) if !t.is_empty() => t.to_owned(),
            _ => readable_name(&self.name),
        }
    }
}

/// A chain name as words: the leading number and its hyphen dropped, hyphens to spaces, the first
/// letter upper-case (`010-midi-controllers` -> `Midi controllers`).
pub fn readable_name(name: &str) -> String {
    let name = name.trim();
    let rest = name.trim_start_matches(|c: char| c.is_ascii_digit());
    // Only a number followed by a hyphen is a number prefix; `2fa` keeps its digit.
    let rest = match rest.strip_prefix('-') {
        Some(r) if rest.len() < name.len() && !r.is_empty() => r,
        _ => name,
    };
    let spaced = rest.replace('-', " ");
    let mut chars = spaced.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A slug for a title: lower-case ASCII letters and digits joined by single hyphens. Cyrillic
/// letters are transliterated; a title with nothing left gives `None`.
pub fn slug_from_title(title: &str) -> Option<String> {
    let mut out = String::new();
    let mut gap = false;
    for ch in title.chars().flat_map(char::to_lowercase) {
        let piece: &str = if ch.is_ascii_alphanumeric() {
            &ch.to_string()
        } else {
            translit(ch)
        };
        if piece.is_empty() {
            gap = true;
            continue;
        }
        if gap && !out.is_empty() {
            out.push('-');
        }
        gap = false;
        out.push_str(piece);
    }
    (!out.is_empty()).then_some(out)
}

fn translit(c: char) -> &'static str {
    match c {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "h",
        'ґ' => "g",
        'д' => "d",
        'е' => "e",
        'є' => "ie",
        'ж' => "zh",
        'з' => "z",
        'и' | 'ы' => "y",
        'і' | 'ї' | 'й' => "i",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ф' => "f",
        'х' => "kh",
        'ц' => "ts",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shch",
        'э' => "e",
        'ю' => "iu",
        'я' => "ia",
        'ё' => "e",
        'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => "a",
        'è' | 'é' | 'ê' | 'ë' => "e",
        'ì' | 'í' | 'î' | 'ï' => "i",
        'ò' | 'ó' | 'ô' | 'ö' | 'õ' => "o",
        'ù' | 'ú' | 'û' | 'ü' => "u",
        'ç' => "c",
        'ñ' => "n",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_made_readable() {
        assert_eq!(readable_name("010-midi-controllers"), "Midi controllers");
        assert_eq!(readable_name("android-release"), "Android release");
        assert_eq!(
            readable_name("001-pad-synth-steps-pedals"),
            "Pad synth steps pedals"
        );
        assert_eq!(readable_name("2fa"), "2fa");
        assert_eq!(readable_name("007"), "007");
        assert_eq!(readable_name("phase-7"), "Phase 7");
    }

    #[test]
    fn a_slug_comes_from_the_title() {
        assert_eq!(
            slug_from_title("MIDI controllers").as_deref(),
            Some("midi-controllers")
        );
        assert_eq!(
            slug_from_title("Android release!").as_deref(),
            Some("android-release")
        );
        assert_eq!(slug_from_title("  A -- B  ").as_deref(), Some("a-b"));
        assert_eq!(
            slug_from_title("Андроїд реліз").as_deref(),
            Some("android-reliz")
        );
        assert_eq!(slug_from_title("安卓发布"), None);
        assert_eq!(slug_from_title("!!!"), None);
    }
}
