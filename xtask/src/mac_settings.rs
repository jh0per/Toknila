//! The settings of `cargo xtask mac-release`: names and paths, read from the environment of the
//! command first and then from `mac-release.env` in the maintainer's release folder
//! (`TOKNILA_RELEASE_DIR`, by default `~/.toknila-release`). Nothing of it is in the repository,
//! and no error here ever contains a value: only the setting's name.

use std::path::{Path, PathBuf};

use crate::runner::Redact;

/// The name of the Developer ID Application certificate in the keychain.
pub const IDENTITY: &str = "APPLE_SIGNING_IDENTITY";
/// The keychain profile `xcrun notarytool store-credentials` saved.
pub const PROFILE: &str = "TOKNILA_NOTARY_PROFILE";
/// The updater's private key (the variable the Tauri signer reads).
pub const KEY_PATH: &str = "TAURI_SIGNING_PRIVATE_KEY_PATH";
/// The file holding the updater key's password.
pub const PASSWORD_PATH: &str = "TOKNILA_UPDATER_PASSWORD_PATH";
/// A build folder outside the repository, kept from one release to the next.
pub const TARGET_DIR: &str = "CARGO_TARGET_DIR";
/// The maintainer's release folder.
pub const DIR: &str = "TOKNILA_RELEASE_DIR";
/// The settings file in that folder.
pub const FILE: &str = "mac-release.env";

const NAMES: [&str; 5] = [IDENTITY, PROFILE, KEY_PATH, PASSWORD_PATH, TARGET_DIR];
const WHERE: &str = "in the environment or in mac-release.env of the release folder (TOKNILA_RELEASE_DIR, by default ~/.toknila-release)";

/// Everything the Mac release needs from the maintainer's computer.
#[derive(Debug, Clone)]
pub struct Settings {
    pub identity: String,
    pub profile: String,
    pub key_path: PathBuf,
    pub password_path: PathBuf,
    pub target_dir: Option<PathBuf>,
}

/// `NAME=value` lines; blank lines and `#` comments are skipped. A line that is not `NAME=value`
/// or a name the command does not read is refused by its line number or its name.
fn parse(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            return Err(format!("{FILE} line {} is not NAME=value", i + 1));
        };
        let name = name.trim();
        if !NAMES.contains(&name) {
            return Err(format!(
                "{FILE} line {} names {name}, which mac-release does not read (it reads {})",
                i + 1,
                NAMES.join(", ")
            ));
        }
        let value = value.trim().trim_matches('"');
        out.push((name.to_owned(), value.to_owned()));
    }
    Ok(out)
}

/// `~/x` with the home folder put in.
fn expand(value: &str, home: Option<&Path>) -> PathBuf {
    match (value.strip_prefix("~/"), home) {
        (Some(rest), Some(h)) => h.join(rest),
        _ => PathBuf::from(value),
    }
}

impl Settings {
    /// Reads the settings. `env` looks a variable up (the process environment, or a test's map);
    /// `home` is the home folder; `repo` the repository, which the build folder must lie outside.
    pub fn load(
        env: &dyn Fn(&str) -> Option<String>,
        home: Option<&Path>,
        repo: &Path,
    ) -> Result<Settings, String> {
        let set = |n: &str| env(n).filter(|v| !v.trim().is_empty());
        let dir = match set(DIR) {
            Some(d) => expand(&d, home),
            None => home
                .map(|h| h.join(".toknila-release"))
                .ok_or("no home folder: set TOKNILA_RELEASE_DIR")?,
        };
        let file = dir.join(FILE);
        let from_file = match std::fs::read_to_string(&file) {
            Ok(text) => parse(&text)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(format!("cannot read {FILE} of the release folder: {e}")),
        };
        let get = |n: &str| {
            set(n).or_else(|| {
                from_file
                    .iter()
                    .rev()
                    .find(|(k, v)| k == n && !v.is_empty())
                    .map(|(_, v)| v.clone())
            })
        };
        let need = |n: &str, what: &str| {
            get(n).ok_or_else(|| format!("{n} is not set: name {what} {WHERE}"))
        };
        let identity = need(IDENTITY, "the Developer ID Application certificate")?;
        if !identity.starts_with("Developer ID Application:") {
            return Err(format!(
                "{IDENTITY} must name a Developer ID Application certificate (its name starts with \"Developer ID Application:\")"
            ));
        }
        let profile = need(
            PROFILE,
            "the notary profile saved with xcrun notarytool store-credentials",
        )?;
        let path_of =
            |n: &str, default: &str| get(n).map_or_else(|| dir.join(default), |v| expand(&v, home));
        let key_path = path_of(KEY_PATH, "updater.key");
        let password_path = path_of(PASSWORD_PATH, "updater.password");
        for (name, path, default) in [
            (KEY_PATH, &key_path, "updater.key"),
            (PASSWORD_PATH, &password_path, "updater.password"),
        ] {
            if !path.is_file() {
                return Err(format!(
                    "the file {name} names (by default {default} in the release folder) does not exist"
                ));
            }
        }
        let target_dir = get(TARGET_DIR).map(|v| expand(&v, home));
        if let Some(t) = &target_dir {
            if !t.is_absolute() {
                return Err(format!("{TARGET_DIR} must be an absolute path"));
            }
            if t.starts_with(repo) {
                return Err(format!(
                    "{TARGET_DIR} is inside the repository; name a folder outside it, so the next release's worktree reuses the build"
                ));
            }
        }
        Ok(Settings {
            identity,
            profile,
            key_path,
            password_path,
            target_dir,
        })
    }

    /// The updater key's password, read from its file (the trailing line break dropped).
    pub fn password(&self) -> Result<String, String> {
        std::fs::read_to_string(&self.password_path)
            .map(|p| p.trim_end_matches(['\n', '\r']).to_owned())
            .map_err(|e| format!("cannot read the file {PASSWORD_PATH} names: {}", e.kind()))
    }

    /// The values a log must never show: the identity, its team ID, the profile, the key's path
    /// and the password.
    pub fn redact(&self, password: &str) -> Redact {
        let mut r = Redact::default();
        r.add(IDENTITY, &self.identity);
        if let Some(team) = self
            .identity
            .rsplit_once('(')
            .and_then(|(_, t)| t.strip_suffix(')'))
        {
            r.add("team ID", team);
        }
        r.add(PROFILE, &self.profile);
        r.add(KEY_PATH, &self.key_path.to_string_lossy());
        r.add("updater key password", password);
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const ID: &str = "Developer ID Application: Test Person (TEAM123456)";

    struct Folder {
        dir: PathBuf,
    }

    impl Folder {
        fn new(name: &str) -> Folder {
            let dir =
                std::env::temp_dir().join(format!("xtask-settings-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("temp folder");
            std::fs::write(dir.join("updater.key"), "fake key").expect("key");
            std::fs::write(dir.join("updater.password"), "fake-password\n").expect("password");
            Folder { dir }
        }
        fn load(&self, vars: &[(&str, &str)], file: Option<&str>) -> Result<Settings, String> {
            if let Some(text) = file {
                std::fs::write(self.dir.join(FILE), text).expect("settings file");
            }
            let mut map: HashMap<String, String> = vars
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect();
            map.insert(DIR.into(), self.dir.to_string_lossy().into_owned());
            Settings::load(&|n| map.get(n).cloned(), None, &self.repo())
        }
        /// An absolute path on every system, so the "inside the repository" rule is what fails.
        fn repo(&self) -> PathBuf {
            self.dir.join("repo-of-toknila")
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn the_file_gives_the_settings_and_the_environment_wins() {
        let f = Folder::new("file");
        let text = format!("# names only\n{IDENTITY}={ID}\n{PROFILE}=from-file\n");
        let s = f
            .load(&[(PROFILE, "from-env")], Some(&text))
            .expect("loads");
        assert_eq!(s.identity, ID);
        assert_eq!(s.profile, "from-env");
        assert_eq!(s.key_path, f.dir.join("updater.key"));
        assert_eq!(s.password().expect("reads"), "fake-password");
        assert_eq!(s.target_dir, None);
    }

    #[test]
    fn every_refusal_names_the_setting_and_never_its_value() {
        let f = Folder::new("refusals");
        let inside = f.repo().join("target").to_string_lossy().into_owned();
        let cases: Vec<(Vec<(&str, &str)>, &str)> = vec![
            (vec![], "APPLE_SIGNING_IDENTITY is not set"),
            (
                vec![(IDENTITY, "Apple Development: Secret Name (TEAMSECRET)")],
                "must name a Developer ID Application",
            ),
            (vec![(IDENTITY, ID)], "TOKNILA_NOTARY_PROFILE is not set"),
            (
                vec![
                    (IDENTITY, ID),
                    (PROFILE, "p"),
                    (KEY_PATH, "/no/such/secret-key"),
                ],
                "the file TAURI_SIGNING_PRIVATE_KEY_PATH names",
            ),
            (
                vec![
                    (IDENTITY, ID),
                    (PROFILE, "p"),
                    (TARGET_DIR, "relative/secret-dir"),
                ],
                "CARGO_TARGET_DIR must be an absolute path",
            ),
            (
                vec![(IDENTITY, ID), (PROFILE, "p"), (TARGET_DIR, &inside)],
                "CARGO_TARGET_DIR is inside the repository",
            ),
        ];
        for (vars, want) in cases {
            let err = f.load(&vars, None).expect_err(want);
            assert!(err.contains(want), "{err}");
            for (_, value) in &vars {
                if value.len() > 1 {
                    assert!(!err.contains(value), "{err} shows a value");
                }
            }
        }
        let err = f
            .load(&[], Some("SECRET_THING=x\n"))
            .expect_err("unknown name");
        assert!(
            err.contains("line 1 names SECRET_THING, which mac-release does not read"),
            "{err}"
        );
        let err = f.load(&[], Some("just a value\n")).expect_err("no =");
        assert_eq!(err, "mac-release.env line 1 is not NAME=value");
    }

    #[test]
    fn the_redaction_hides_the_identity_its_team_the_profile_the_key_and_the_password() {
        let f = Folder::new("redact");
        let s = f
            .load(&[(IDENTITY, ID), (PROFILE, "fake-notary-profile")], None)
            .expect("loads");
        let r = s.redact("fake-password");
        let key = s.key_path.to_string_lossy().into_owned();
        let text = format!("{ID} TEAM123456 fake-notary-profile {key} fake-password");
        assert_eq!(
            r.apply(&text),
            "<APPLE_SIGNING_IDENTITY> <team ID> <TOKNILA_NOTARY_PROFILE> <TAURI_SIGNING_PRIVATE_KEY_PATH> <updater key password>"
        );
    }
}
