//! What a form suggests for a new project from the folder of its repository: the one place that
//! derives a key and a name, so every client shows the same.

use crate::ProjectKey;

/// The last part of a folder path, for either kind of separator; `None` for a path without one.
fn folder_name(path: &str) -> Option<&str> {
    path.trim()
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .map(str::trim)
        .filter(|n| !n.is_empty() && !n.ends_with(':'))
}

/// The project name a folder suggests: its base name (`/work/Toknila/` gives `Toknila`).
pub fn project_name(path: &str) -> Option<String> {
    folder_name(path).map(str::to_owned)
}

/// The project key a folder suggests: the base name in uppercase, letters and digits only,
/// without leading digits, cut to 10 characters. `None` when fewer than 2 characters remain.
pub fn project_key(path: &str) -> Option<String> {
    let key: String = folder_name(path)?
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .skip_while(char::is_ascii_digit)
        .take(10)
        .collect();
    key.parse::<ProjectKey>().ok().map(|k| k.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_is_the_folder_name_in_uppercase() {
        assert_eq!(
            project_key("/Users/me/Work/Toknila").as_deref(),
            Some("TOKNILA")
        );
        assert_eq!(
            project_key("/Users/me/Work/Toknila/").as_deref(),
            Some("TOKNILA")
        );
        assert_eq!(project_key("C:\\Work\\my-app").as_deref(), Some("MYAPP"));
    }

    #[test]
    fn the_key_starts_with_a_letter_and_is_at_most_ten_characters() {
        assert_eq!(project_key("/w/42-answers").as_deref(), Some("ANSWERS"));
        assert_eq!(
            project_key("/w/a-very-long-folder-name").as_deref(),
            Some("AVERYLONGF")
        );
        assert_eq!(project_key("/w/web2go").as_deref(), Some("WEB2GO"));
    }

    #[test]
    fn a_folder_that_leaves_no_valid_key_suggests_none() {
        assert_eq!(project_key("/w/x"), None);
        assert_eq!(project_key("/w/123"), None);
        assert_eq!(project_key("/w/--"), None);
        assert_eq!(project_key(""), None);
        assert_eq!(project_key("/"), None);
        assert_eq!(project_key("C:\\"), None);
    }

    #[test]
    fn the_name_is_the_base_name_as_written() {
        assert_eq!(
            project_name("/Users/me/Work/Toknila").as_deref(),
            Some("Toknila")
        );
        assert_eq!(
            project_name("/Users/me/my repo/").as_deref(),
            Some("my repo")
        );
        assert_eq!(
            project_name("C:\\Work\\Toknila").as_deref(),
            Some("Toknila")
        );
        assert_eq!(project_name("/"), None);
        assert_eq!(project_name(" "), None);
    }
}
