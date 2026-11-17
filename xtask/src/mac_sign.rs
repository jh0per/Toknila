//! One architecture of the Mac release, after `Toknila.app` is assembled: sign it with the
//! Developer ID under the hardened runtime, make and sign the dmg, have Apple notarize it, staple
//! the app and the dmg, then pack the updater archive from the stapled app and sign it with the
//! maintainer's updater key. Every tool runs through an [`Exec`], so the order and the arguments
//! are tested with a recording runner and with fake programs.

use std::path::{Path, PathBuf};

use crate::mac_settings::{IDENTITY, KEY_PATH};
use crate::runner::{Cmd, Exec, check};
use crate::{app, updates};

/// What signing needs: the identity, the notary profile, the entitlements of the shell, the
/// updater key's path and its password.
#[derive(Debug)]
pub struct Signing<'a> {
    pub identity: &'a str,
    pub profile: &'a str,
    pub entitlements: &'a Path,
    pub key_path: &'a Path,
    pub password: &'a str,
}

/// The entitlements of the shell (the bundle's main executable).
pub fn entitlements(root: &Path) -> PathBuf {
    app::shell_dir(root).join("entitlements.plist")
}

/// Why a `codesign` failed, in one sentence, when its error output says so.
fn codesign_cause(err: &str) -> Option<String> {
    let e = err.to_lowercase();
    if e.contains("errsecinternalcomponent")
        || e.contains("interaction is not allowed")
        || e.contains("canceled")
        || e.contains("cancelled")
    {
        return Some(
            "codesign could not use the certificate's private key: the keychain window that asks for it was denied or not answered; run `cargo xtask mac-release <tag> --no-upload` at the Mac, press Always Allow, then run the Build step again".to_owned(),
        );
    }
    if e.contains("no identity found") || e.contains("could not be found in the keychain") {
        return Some(format!(
            "codesign found no certificate named by {IDENTITY} in the keychain: install the Developer ID Application certificate (double-click it in Finder) or correct the name"
        ));
    }
    None
}

/// Runs a `codesign` and turns a keychain refusal into its sentence.
fn codesign(exec: &mut dyn Exec, cmd: Cmd) -> Result<(), String> {
    let ran = exec.run(&cmd)?;
    if ran.ok {
        return Ok(());
    }
    if let Some(cause) = codesign_cause(&ran.err) {
        return Err(cause);
    }
    let shown = cmd.shown(exec.redact());
    Err(format!(
        "`{}` exited with {}",
        shown.trim_start_matches("$ "),
        ran.status
    ))
}

fn sign_cmd(s: &Signing, dir: &Path, what: &Path, entitlements: bool) -> Cmd {
    let mut cmd =
        Cmd::new("codesign", dir).args(["--force", "--options", "runtime", "--timestamp"]);
    if entitlements {
        cmd = cmd.args(["--entitlements"]).path(s.entitlements);
    }
    cmd.args(["--sign", s.identity]).path(what)
}

/// Signs the helpers, then the bundle (its main executable gets the entitlements), and verifies.
pub fn sign_bundle(exec: &mut dyn Exec, s: &Signing, bundle: &Path) -> Result<(), String> {
    let dir = bundle.parent().unwrap_or(bundle);
    let macos = bundle.join("Contents/MacOS");
    for helper in ["toknila", "tok"] {
        codesign(exec, sign_cmd(s, dir, &macos.join(helper), false))?;
    }
    codesign(exec, sign_cmd(s, dir, bundle, true))?;
    check(
        exec,
        &Cmd::new("codesign", dir)
            .args(["--verify", "--strict", "--verbose=2"])
            .path(bundle),
    )?;
    Ok(())
}

/// The value of `key` in what `notarytool` printed: JSON (`"status": "Accepted"`) or text
/// (`status: Accepted`), the last one found.
pub fn notary_field(out: &str, key: &str) -> Option<String> {
    let quoted = format!("\"{key}\"");
    if let Some(at) = out.rfind(&quoted) {
        let rest = out[at + quoted.len()..].trim_start().strip_prefix(':')?;
        let rest = rest.trim_start().strip_prefix('"')?;
        return rest.split('"').next().map(str::to_owned);
    }
    let prefix = format!("{key}: ");
    out.lines()
        .rev()
        .find_map(|l| l.trim().strip_prefix(&prefix).map(|v| v.trim().to_owned()))
}

/// Submits `dmg` to Apple's notary service and waits. Anything but Accepted prints the log of the
/// submission and fails naming the file.
pub fn notarize(exec: &mut dyn Exec, s: &Signing, dmg: &Path) -> Result<(), String> {
    let dir = dmg.parent().unwrap_or(dmg);
    let name = dmg
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let submit = Cmd::new("xcrun", dir)
        .args(["notarytool", "submit"])
        .path(dmg)
        .args([
            "--keychain-profile",
            s.profile,
            "--wait",
            "--output-format",
            "json",
        ]);
    let ran = exec.run(&submit)?;
    let status = notary_field(&ran.out, "status");
    if status.as_deref() == Some("Accepted") {
        exec.say(&format!("xtask mac-release: Apple notarized {name}"));
        return Ok(());
    }
    let id = notary_field(&ran.out, "id");
    if let Some(id) = &id {
        let log =
            Cmd::new("xcrun", dir).args(["notarytool", "log", id, "--keychain-profile", s.profile]);
        let _ = exec.run(&log)?;
    }
    Err(match (status, id) {
        (Some(st), Some(id)) => format!(
            "Apple's notary service answered {st} for {name} (submission {id}); its log is above"
        ),
        _ => format!(
            "the submission of {name} to Apple's notary service did not finish ({}); see the lines above",
            ran.status
        ),
    })
}

/// Staples the tickets to the app and the dmg, validates the dmg and asks Gatekeeper about the app.
pub fn staple(exec: &mut dyn Exec, bundle: &Path, dmg: &Path) -> Result<(), String> {
    let dir = dmg.parent().unwrap_or(dmg);
    for what in [bundle, dmg] {
        check(
            exec,
            &Cmd::new("xcrun", dir)
                .args(["stapler", "staple"])
                .path(what),
        )?;
    }
    check(
        exec,
        &Cmd::new("xcrun", dir)
            .args(["stapler", "validate"])
            .path(dmg),
    )?;
    let ran = check(
        exec,
        &Cmd::new("spctl", dir).args(["-a", "-vv"]).path(bundle),
    )?;
    let said = format!("{}{}", ran.out, ran.err);
    if !said.contains("Notarized Developer ID") {
        return Err("Gatekeeper accepted the app but did not call it notarized (spctl did not say \"Notarized Developer ID\")".to_owned());
    }
    Ok(())
}

/// Packs the stapled app into the updater archive and signs it with the maintainer's key: the
/// key's path and the password reach the signer only through its environment.
pub fn updater_archive(
    exec: &mut dyn Exec,
    s: &Signing,
    out: &Path,
    version: &str,
    arch: &str,
) -> Result<(PathBuf, PathBuf), String> {
    let archive = updates::pack(exec, out, version, arch)?;
    let cmd = updates::signer(&archive, out)
        .env(KEY_PATH, &s.key_path.to_string_lossy())
        .env("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", s.password)
        .env_remove("TAURI_SIGNING_PRIVATE_KEY");
    check(exec, &cmd)?;
    let sig = PathBuf::from(format!("{}.sig", archive.to_string_lossy()));
    if !sig.is_file() {
        return Err(format!("the signer wrote no {}", file_name(&sig)));
    }
    Ok((archive, sig))
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Everything after the assembly for one architecture; returns the dmg, the archive and its
/// signature, in that order.
pub fn one_target(
    exec: &mut dyn Exec,
    s: &Signing,
    out: &Path,
    version: &str,
    arch: &str,
) -> Result<Vec<PathBuf>, String> {
    let bundle = out.join("Toknila.app");
    sign_bundle(exec, s, &bundle)?;
    let dmg = app::make_dmg(exec, out, version, arch)?;
    codesign(
        exec,
        Cmd::new("codesign", out)
            .args(["--force", "--timestamp", "--sign", s.identity])
            .path(&dmg),
    )?;
    notarize(exec, s, &dmg)?;
    staple(exec, &bundle, &dmg)?;
    let (archive, sig) = updater_archive(exec, s, out, version, arch)?;
    Ok(vec![dmg, archive, sig])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notarytool_answers_are_read_as_json_or_as_text() {
        let json = r#"{"id":"2efe2717-52ef","message":"Processing complete","status":"Accepted"}"#;
        assert_eq!(notary_field(json, "status").as_deref(), Some("Accepted"));
        assert_eq!(notary_field(json, "id").as_deref(), Some("2efe2717-52ef"));
        let text = "Processing complete\n  id: abc-1\n  status: Invalid\n";
        assert_eq!(notary_field(text, "status").as_deref(), Some("Invalid"));
        assert_eq!(notary_field(text, "id").as_deref(), Some("abc-1"));
        assert_eq!(notary_field("Error: HTTP status code: 401", "status"), None);
    }

    #[test]
    fn a_keychain_refusal_and_a_missing_certificate_are_told_by_their_cause() {
        for err in [
            "Toknila.app: errSecInternalComponent",
            "Toknila.app: User interaction is not allowed.",
            "Toknila.app: The operation was canceled by the user",
        ] {
            let cause = codesign_cause(err).expect(err);
            assert!(
                cause.contains("the keychain window that asks for it was denied or not answered"),
                "{cause}"
            );
            assert!(cause.contains("Always Allow"), "{cause}");
        }
        let cause = codesign_cause("error: The specified item could not be found in the keychain.")
            .expect("missing certificate");
        assert!(
            cause.starts_with("codesign found no certificate named by APPLE_SIGNING_IDENTITY"),
            "{cause}"
        );
        assert_eq!(
            codesign_cause("Toknila.app: a sealed resource is missing or invalid"),
            None
        );
    }
}
