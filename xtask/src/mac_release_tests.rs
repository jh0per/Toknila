//! Tests of `cargo xtask mac-release`: a recording runner for the order, the arguments and the
//! failures; on macOS, fake programs on a temporary `PATH` run through the real runner, to show
//! that no setting's value reaches what is printed.

use super::*;
use crate::runner::{Ran, Redact};

const ID: &str = "Developer ID Application: Test Person (TEAM123456)";
const PROFILE: &str = "fake-notary-profile";
const PASSWORD: &str = "fake-updater-password";
const TAG: &str = "v9.9.9";

/// A temporary folder that is removed at the end.
struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Temp {
        let dir = std::env::temp_dir().join(format!("xtask-mac-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp folder");
        Temp(dir)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn signing<'a>(root: &'a Path, key: &'a Path) -> Signing<'a> {
    Signing {
        identity: ID,
        profile: PROFILE,
        entitlements: root,
        key_path: key,
        password: PASSWORD,
    }
}

/// The build step of a test: an assembled-looking bundle, no cargo.
fn fake_build(built: &mut Vec<String>) -> impl FnMut(&str, &Path) -> Result<(), String> + '_ {
    move |triple, out| {
        built.push(triple.to_owned());
        let macos = out.join("Toknila.app/Contents/MacOS");
        std::fs::create_dir_all(&macos).map_err(|e| e.to_string())?;
        for name in ["toknila", "tok", "toknila-desktop"] {
            std::fs::write(macos.join(name), "binary").map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

// `line`, `expected` and `Fake::programs` serve the unix-only tests below only, so they carry the
// same gate (clippy denies dead code on Windows).
#[cfg(unix)]
fn line(s: &[&str]) -> Vec<String> {
    s.iter().map(|a| (*a).to_owned()).collect()
}

/// Every command of a release that uploads to a tag with no release yet, as program and arguments.
#[cfg(unix)]
fn expected(root: &Path, entitlements: &Path) -> Vec<Vec<String>> {
    let p = |p: &Path| p.to_string_lossy().into_owned();
    let view = line(&[
        "gh",
        "release",
        "view",
        TAG,
        "--repo",
        "jh0per/Toknila",
        "--json",
        "isDraft,assets",
        "--jq",
        ".isDraft, .assets[].name",
    ]);
    let mut all = vec![
        line(&["xcrun", "--version"]),
        line(&["gh", "--version"]),
        line(&["rustup", "target", "list", "--installed"]),
        line(&[
            "gh",
            "api",
            "repos/jh0per/Toknila/git/ref/tags/v9.9.9",
            "--jq",
            ".ref",
        ]),
        view.clone(),
        line(&[
            "xcrun",
            "notarytool",
            "history",
            "--keychain-profile",
            PROFILE,
        ]),
    ];
    let (mut files, mut sigs) = (Vec::new(), Vec::new());
    let ent = p(entitlements);
    for arch in ["aarch64", "x86_64"] {
        let out = root.join("dist/mac-release").join(arch);
        let app = p(&out.join("Toknila.app"));
        let dmg = p(&out.join(format!("Toknila-9.9.9-{arch}.dmg")));
        let archive = p(&out.join(format!("Toknila-9.9.9-{arch}.app.tar.gz")));
        let stage = p(&out.join("dmg"));
        let staged = p(&out.join("dmg").join("Toknila.app"));
        let (app, dmg, archive) = (app.as_str(), dmg.as_str(), archive.as_str());
        let sign = ["codesign", "--force", "--options", "runtime", "--timestamp"];
        for helper in ["toknila", "tok"] {
            let bin = p(&out.join("Toknila.app/Contents/MacOS").join(helper));
            all.push(line(&[&sign[..], &["--sign", ID, bin.as_str()]].concat()));
        }
        all.push(line(
            &[
                &sign[..],
                &["--entitlements", ent.as_str(), "--sign", ID, app],
            ]
            .concat(),
        ));
        all.push(line(&[
            "codesign",
            "--verify",
            "--strict",
            "--verbose=2",
            app,
        ]));
        all.push(line(&["ditto", app, staged.as_str()]));
        all.push(line(&[
            "hdiutil",
            "create",
            "-volname",
            "Toknila",
            "-srcfolder",
            stage.as_str(),
            "-ov",
            "-format",
            "UDZO",
            dmg,
        ]));
        all.push(line(&[
            "codesign",
            "--force",
            "--timestamp",
            "--sign",
            ID,
            dmg,
        ]));
        all.push(line(&[
            "xcrun",
            "notarytool",
            "submit",
            dmg,
            "--keychain-profile",
            PROFILE,
            "--wait",
            "--output-format",
            "json",
        ]));
        all.push(line(&["xcrun", "stapler", "staple", app]));
        all.push(line(&["xcrun", "stapler", "staple", dmg]));
        all.push(line(&["xcrun", "stapler", "validate", dmg]));
        all.push(line(&["spctl", "-a", "-vv", app]));
        all.push(line(&["tar", "-czf", archive, "Toknila.app"]));
        all.push(line(&[
            "npx",
            "--yes",
            "@tauri-apps/cli@2.12.4",
            "signer",
            "sign",
            archive,
        ]));
        files.extend([dmg.to_owned(), archive.to_owned()]);
        sigs.push(format!("{archive}.sig"));
    }
    all.push(view);
    all.push(line(&[
        "gh",
        "release",
        "create",
        TAG,
        "--repo",
        "jh0per/Toknila",
        "--draft",
        "--verify-tag",
        "--title",
        "Toknila v9.9.9",
        "--notes",
        "The Mac files are uploaded; the Release workflow adds the rest and publishes.",
    ]));
    // The signatures go last: CI publishes only when all six files are there.
    for set in [files, sigs] {
        let mut upload = line(&[
            "gh",
            "release",
            "upload",
            TAG,
            "--repo",
            "jh0per/Toknila",
            "--clobber",
        ]);
        upload.extend(set);
        all.push(upload);
    }
    all
}

/// The recording runner: answers like the real tools and does what the next step needs
/// (the dmg, the archive and the signature appear).
#[derive(Default)]
struct Fake {
    calls: Vec<Cmd>,
    said: Vec<String>,
    redact: Redact,
    /// `pushed`, `missing` (404) or anything else (another gh error).
    tag: &'static str,
    /// `missing`, `draft` (a rerun: the aarch64 dmg and signature are there) or `published`.
    release: &'static str,
    targets: &'static str,
    notary: &'static str,
    codesign_err: Option<&'static str>,
    /// What `notarytool history` fails with; `None` is a readable, accepted profile.
    history_err: Option<&'static str>,
    /// What `notarytool submit` fails with (exit status 69); `None` is the answer in `notary`.
    submit_err: Option<&'static str>,
}

impl Fake {
    fn new() -> Fake {
        Fake {
            tag: "pushed",
            release: "missing",
            targets: "aarch64-apple-darwin\nx86_64-apple-darwin\n",
            notary: r#"{"id":"sub-1","message":"Processing complete","status":"Accepted"}"#,
            ..Fake::default()
        }
    }
    #[cfg(unix)]
    fn programs(&self) -> Vec<Vec<String>> {
        self.calls
            .iter()
            .map(|c| [vec![c.program.clone()], c.args.clone()].concat())
            .collect()
    }
}

impl Exec for Fake {
    fn run(&mut self, cmd: &Cmd) -> Result<Ran, String> {
        self.calls.push(cmd.clone());
        let a: Vec<&str> = cmd.args.iter().map(String::as_str).collect();
        let ok = |out: &str| {
            Ok(Ran {
                ok: true,
                status: "exit status: 0".into(),
                out: out.into(),
                err: String::new(),
            })
        };
        let fail = |err: &str| {
            Ok(Ran {
                ok: false,
                status: "exit status: 1".into(),
                out: String::new(),
                err: err.into(),
            })
        };
        match (cmd.program.as_str(), a.as_slice()) {
            ("rustup", _) => ok(self.targets),
            ("gh", ["api", ..]) => match self.tag {
                "pushed" => ok("refs/tags/v9.9.9\n"),
                "missing" => fail("gh: Not Found (HTTP 404)"),
                _ => fail("error connecting to api.github.com\ncheck your internet connection"),
            },
            ("gh", ["release", "view", ..]) => match self.release {
                "missing" => fail("release not found"),
                "draft" => {
                    ok("true\nToknila-9.9.9-aarch64.dmg\nToknila-9.9.9-aarch64.app.tar.gz.sig\n")
                }
                _ => ok("false\nToknila-9.9.9-aarch64.dmg\n"),
            },
            ("codesign", ["--force", ..]) if self.codesign_err.is_some() => {
                fail(self.codesign_err.unwrap_or_default())
            }
            ("hdiutil", [.., last]) | ("tar", [_, last, _]) => {
                std::fs::write(last, "made").map_err(|e| e.to_string())?;
                ok("")
            }
            ("npx", [.., file]) => {
                std::fs::write(format!("{file}.sig"), "SIG").map_err(|e| e.to_string())?;
                ok("")
            }
            ("xcrun", ["notarytool", "history", ..]) => match self.history_err {
                Some(err) => fail(err),
                None => ok("Successfully received submission history.\n"),
            },
            ("xcrun", ["notarytool", "submit", ..]) if self.submit_err.is_some() => {
                fail(self.submit_err.unwrap_or_default())
            }
            ("xcrun", ["notarytool", "submit", ..]) => ok(self.notary),
            ("spctl", _) => Ok(Ran {
                ok: true,
                status: "exit status: 0".into(),
                out: String::new(),
                err: "accepted\nsource=Notarized Developer ID\n".into(),
            }),
            _ => ok(""),
        }
    }
    fn redact(&self) -> &Redact {
        &self.redact
    }
    fn say(&mut self, line: &str) {
        self.said.push(line.to_owned());
    }
}

fn args(upload: bool) -> Args {
    Args {
        tag: TAG.into(),
        upload,
    }
}

// The dmg's link to /Applications is a unix symlink (`make_dmg`), so the pipeline runs on unix.
#[cfg(unix)]
#[test]
fn a_release_builds_both_targets_signs_notarizes_staples_and_uploads_in_order() {
    let t = Temp::new("order");
    let (ent, key) = (t.0.join("entitlements.plist"), t.0.join("updater.key"));
    let mut fake = Fake::new();
    let mut built = Vec::new();
    let s = Signing {
        entitlements: &ent,
        ..signing(&t.0, &key)
    };
    let files = release(
        &mut fake,
        &t.0,
        &args(true),
        "9.9.9",
        &s,
        &mut fake_build(&mut built),
    )
    .expect("the release passes");
    assert_eq!(built, TARGETS);
    assert_eq!(fake.programs(), expected(&t.0, &ent));
    assert_eq!(files.len(), 6);
    let npx = fake
        .calls
        .iter()
        .find(|c| c.program == "npx")
        .expect("the signer ran");
    let env: Vec<(&str, &str)> = npx
        .env
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert!(
        env.contains(&("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", PASSWORD)),
        "{env:?}"
    );
    assert!(
        env.contains(&("TAURI_SIGNING_PRIVATE_KEY_PATH", &*key.to_string_lossy())),
        "{env:?}"
    );
    assert_eq!(npx.env_remove, ["TAURI_SIGNING_PRIVATE_KEY"]);
    assert!(
        !npx.args.iter().any(|a| a.contains(PASSWORD)),
        "the password is never an argument"
    );
}

/// A rerun on a draft: no new release, and the old signature goes first, so the draft lacks a
/// signature (and CI waits) until the new archives and then the new signatures are up.
#[cfg(unix)]
#[test]
fn a_draft_is_uploaded_to_without_creating_one_and_no_upload_touches_no_github() {
    let t = Temp::new("draft");
    let key = t.0.join("k");
    let mut fake = Fake {
        release: "draft",
        ..Fake::new()
    };
    release(
        &mut fake,
        &t.0,
        &args(true),
        "9.9.9",
        &signing(&t.0, &key),
        &mut fake_build(&mut Vec::new()),
    )
    .expect("passes");
    let mut want = expected(&t.0, &t.0);
    let create = want.len() - 3;
    assert_eq!(want[create][..3], ["gh", "release", "create"]);
    want[create] = line(&[
        "gh",
        "release",
        "delete-asset",
        TAG,
        "Toknila-9.9.9-aarch64.app.tar.gz.sig",
        "--repo",
        "jh0per/Toknila",
        "--yes",
    ]);
    assert_eq!(fake.programs(), want);

    let mut fake = Fake::new();
    release(
        &mut fake,
        &t.0,
        &args(false),
        "9.9.9",
        &signing(&t.0, &key),
        &mut fake_build(&mut Vec::new()),
    )
    .expect("passes");
    assert!(
        !fake.calls.iter().any(|c| c.program == "gh"),
        "--no-upload calls no gh"
    );
    assert!(
        fake.calls
            .iter()
            .any(|c| c.args.get(1).is_some_and(|a| a == "history")),
        "--no-upload still checks the notary profile"
    );
    assert_eq!(
        fake.said.last().map(String::as_str),
        Some("xtask mac-release: --no-upload, so nothing went to GitHub")
    );
}

#[test]
fn preflight_refuses_in_seconds_before_any_build() {
    let t = Temp::new("preflight");
    let key = t.0.join("k");
    let cases: Vec<(Fake, Args, &str)> = vec![
        (
            Fake {
                release: "published",
                ..Fake::new()
            },
            args(true),
            "the release v9.9.9 is already published; mac-release does not replace its files",
        ),
        (
            Fake {
                targets: "aarch64-apple-darwin\n",
                ..Fake::new()
            },
            args(true),
            "the Rust target x86_64-apple-darwin is not installed: run rustup target add x86_64-apple-darwin",
        ),
        (
            Fake {
                targets: "",
                ..Fake::new()
            },
            args(true),
            "the Rust targets aarch64-apple-darwin and x86_64-apple-darwin are not installed: run rustup target add aarch64-apple-darwin x86_64-apple-darwin",
        ),
        (
            Fake {
                tag: "missing",
                ..Fake::new()
            },
            args(true),
            "the tag v9.9.9 is not on GitHub: push it (git push origin v9.9.9), then run the Build step again",
        ),
        (
            Fake {
                tag: "offline",
                ..Fake::new()
            },
            args(true),
            "gh could not look up the tag v9.9.9 on GitHub: check your internet connection",
        ),
        (
            Fake::new(),
            Args {
                tag: "v1.0.0".into(),
                upload: true,
            },
            "the tag v1.0.0 is not v9.9.9, the version of this checkout",
        ),
    ];
    for (mut fake, a, want) in cases {
        let mut built = Vec::new();
        let err = release(
            &mut fake,
            &t.0,
            &a,
            "9.9.9",
            &signing(&t.0, &key),
            &mut fake_build(&mut built),
        )
        .expect_err(want);
        assert!(err.starts_with(want), "{err}");
        assert!(built.is_empty(), "nothing was built for: {want}");
        assert!(!fake.calls.iter().any(|c| c.program == "codesign"));
    }
}

const NO_ITEM: &str = "Error: No Keychain password item found for profile: fake-notary-profile\n\nRun 'notarytool store-credentials' to create another credential profile.";

fn history_cmd() -> Vec<String> {
    line_any(&[
        "xcrun",
        "notarytool",
        "history",
        "--keychain-profile",
        PROFILE,
    ])
}

fn line_any(s: &[&str]) -> Vec<String> {
    s.iter().map(|a| (*a).to_owned()).collect()
}

/// The check of the notary profile runs before the first build, with and without `--no-upload`,
/// and a profile that cannot be read names the setting and what to do.
#[test]
fn an_unreadable_notary_profile_stops_the_release_before_any_build() {
    let t = Temp::new("history");
    let key = t.0.join("k");
    for upload in [true, false] {
        let mut fake = Fake {
            history_err: Some(NO_ITEM),
            ..Fake::new()
        };
        let mut built = Vec::new();
        let err = release(
            &mut fake,
            &t.0,
            &args(upload),
            "9.9.9",
            &signing(&t.0, &key),
            &mut fake_build(&mut built),
        )
        .expect_err("the profile cannot be read");
        assert_eq!(
            err,
            "TOKNILA_NOTARY_PROFILE names a notary profile that this Mac's keychain does not hold: check the name in the settings file, or save the profile with `xcrun notarytool store-credentials`"
        );
        assert!(built.is_empty(), "nothing was built (upload {upload})");
        let all: Vec<Vec<String>> = fake
            .calls
            .iter()
            .map(|c| [vec![c.program.clone()], c.args.clone()].concat())
            .collect();
        assert!(all.contains(&history_cmd()), "{all:?}");
        assert!(!all.iter().any(|c| c[0] == "codesign"));
        if !upload {
            assert!(!all.iter().any(|c| c[0] == "gh"), "--no-upload calls no gh");
        }
    }
}

#[test]
fn a_refused_login_or_no_network_says_so_with_what_notarytool_printed() {
    let t = Temp::new("refused");
    let key = t.0.join("k");
    for said in [
        "Error: HTTP status code: 401. Invalid credentials. Username or password is incorrect.",
        "Error: The Internet connection appears to be offline.",
    ] {
        let mut fake = Fake {
            history_err: Some(said),
            ..Fake::new()
        };
        let mut built = Vec::new();
        let err = release(
            &mut fake,
            &t.0,
            &args(false),
            "9.9.9",
            &signing(&t.0, &key),
            &mut fake_build(&mut built),
        )
        .expect_err("the check fails");
        assert_eq!(
            err,
            format!(
                "Apple's notary service did not accept the notary profile (no network, or a refused login); notarytool printed: {said}"
            )
        );
        assert!(built.is_empty());
    }
}

// The dmg's link to /Applications is a unix symlink (`make_dmg`), so the pipeline runs on unix.
#[cfg(unix)]
#[test]
fn a_profile_that_was_readable_at_the_start_and_not_at_the_submission_says_exactly_that() {
    let t = Temp::new("lost");
    let key = t.0.join("k");
    let mut fake = Fake {
        submit_err: Some(NO_ITEM),
        ..Fake::new()
    };
    let err = release(
        &mut fake,
        &t.0,
        &args(true),
        "9.9.9",
        &signing(&t.0, &key),
        &mut fake_build(&mut Vec::new()),
    )
    .expect_err("the submission fails");
    assert_eq!(
        err,
        "the notary profile was readable when this run started and could not be read now (Toknila-9.9.9-aarch64.dmg was not submitted): unlock this Mac and keep it unlocked while the command runs, then run it again (the release's Build step can be pressed again); if it still fails, save the profile again with `xcrun notarytool store-credentials`"
    );
    assert!(
        fake.calls
            .iter()
            .any(|c| c.args.get(1).is_some_and(|a| a == "history"))
    );
}

#[cfg(unix)]
#[test]
fn an_invalid_notarization_prints_its_log_and_nothing_is_stapled_or_uploaded() {
    let t = Temp::new("invalid");
    let key = t.0.join("k");
    let mut fake = Fake {
        notary: r#"{"id":"sub-9","message":"Processing complete","status":"Invalid"}"#,
        ..Fake::new()
    };
    let err = release(
        &mut fake,
        &t.0,
        &args(true),
        "9.9.9",
        &signing(&t.0, &key),
        &mut fake_build(&mut Vec::new()),
    )
    .expect_err("Invalid fails");
    assert_eq!(
        err,
        "Apple's notary service answered Invalid for Toknila-9.9.9-aarch64.dmg (submission sub-9); its log is above"
    );
    let calls = fake.programs();
    let last = calls.last().expect("calls");
    assert_eq!(
        last,
        &[
            "xcrun",
            "notarytool",
            "log",
            "sub-9",
            "--keychain-profile",
            PROFILE
        ]
    );
    assert!(
        !calls
            .iter()
            .any(|c| c.get(1).is_some_and(|a| a == "stapler"))
    );
    assert!(
        !calls
            .iter()
            .any(|c| c.get(1).is_some_and(|a| a == "release") && c[2] != "view")
    );
}

#[test]
fn a_denied_keychain_window_is_told_in_one_sentence_that_names_no_value() {
    let t = Temp::new("keychain");
    let key = t.0.join("k");
    let mut fake = Fake {
        codesign_err: Some("Toknila.app/Contents/MacOS/toknila: errSecInternalComponent"),
        ..Fake::new()
    };
    let err = release(
        &mut fake,
        &t.0,
        &args(true),
        "9.9.9",
        &signing(&t.0, &key),
        &mut fake_build(&mut Vec::new()),
    )
    .expect_err("codesign fails");
    assert!(err.starts_with("codesign could not use the certificate's private key: the keychain window that asks for it was denied or not answered"), "{err}");
    assert!(
        !err.contains("Test Person") && !err.contains("TEAM123456"),
        "{err}"
    );
}

#[test]
fn the_arguments_are_a_tag_and_an_optional_no_upload() {
    let parse = |v: &[&str]| parse_args(v.iter().map(|s| (*s).to_owned()));
    assert_eq!(
        parse(&["v0.3.8"]),
        Ok(Args {
            tag: "v0.3.8".into(),
            upload: true
        })
    );
    assert_eq!(
        parse(&["v0.3.8", "--no-upload"]),
        Ok(Args {
            tag: "v0.3.8".into(),
            upload: false
        })
    );
    assert_eq!(parse(&[]), Err(USAGE.to_owned()));
    assert!(parse(&["0.3.8"]).is_err());
}

/// The real runner with fake programs on a temporary `PATH`: each records its arguments, and
/// `spctl` and `notarytool` print the identity and the profile as the real ones can. The order
/// must be the same as above, and no printed line may carry a value.
// macOS only: `mac-release` runs only there, and the fakes are `sh` scripts executed right
// after they are written, which Linux can refuse with "Text file busy" under parallel tests.
#[cfg(target_os = "macos")]
#[test]
fn fake_programs_on_a_temporary_path_record_the_order_and_nothing_secret_is_printed() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Arc, Mutex};

    let t = Temp::new("path");
    let (bin, root) = (t.0.join("bin"), t.0.join("repo"));
    std::fs::create_dir_all(&bin).expect("bin");
    std::fs::create_dir_all(&root).expect("repo");
    let record = t.0.join("record");
    let ent = root.join("entitlements.plist");
    let key = t.0.join("updater.key");
    let rec = record.to_string_lossy();
    let head = format!(
        "#!/bin/sh\n{{ printf '%s' \"$(basename \"$0\")\"; for a in \"$@\"; do printf '\\037%s' \"$a\"; done; printf '\\n'; }} >> '{rec}'\nfor a in \"$@\"; do last=$a; done\n"
    );
    let fakes = [
        ("codesign", String::new()),
        ("ditto", String::new()),
        ("hdiutil", ": > \"$last\"\n".to_owned()),
        ("tar", ": > \"$2\"\n".to_owned()),
        (
            "npx",
            format!(
                "printf SIG > \"$last.sig\"\necho \"signer-env $TAURI_SIGNING_PRIVATE_KEY_PASSWORD ${{TAURI_SIGNING_PRIVATE_KEY:-unset}}\" >> '{rec}'\n"
            ),
        ),
        (
            "rustup",
            "echo aarch64-apple-darwin; echo x86_64-apple-darwin\n".to_owned(),
        ),
        (
            "spctl",
            format!(
                "echo \"$last: accepted\" >&2; echo 'source=Notarized Developer ID' >&2; echo 'origin={ID}' >&2\n"
            ),
        ),
        (
            "xcrun",
            format!(
                "if [ \"$2\" = submit ]; then echo \"using {PROFILE} for {ID}\"; echo '{{\"id\":\"sub-1\",\"status\":\"Accepted\"}}'; fi\n"
            ),
        ),
        (
            "gh",
            "if [ \"$1\" = api ]; then echo refs/tags/v9.9.9; fi\nif [ \"$2\" = view ]; then echo 'release not found' >&2; exit 1; fi\n".to_owned(),
        ),
    ];
    for (name, body) in &fakes {
        let path = bin.join(name);
        std::fs::write(&path, format!("{head}{body}exit 0\n")).expect("fake");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let sink = Arc::new(Mutex::new(Vec::new()));
    let mut redact = Redact::default();
    redact.add("APPLE_SIGNING_IDENTITY", ID);
    redact.add("team ID", "TEAM123456");
    redact.add("TOKNILA_NOTARY_PROFILE", PROFILE);
    redact.add("TAURI_SIGNING_PRIVATE_KEY_PATH", &key.to_string_lossy());
    redact.add("updater key password", PASSWORD);
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let mut exec = Process::new(redact)
        .with_path(path.into())
        .with_sink(sink.clone());
    let s = Signing {
        entitlements: &ent,
        ..signing(&root, &key)
    };
    release(
        &mut exec,
        &root,
        &args(true),
        "9.9.9",
        &s,
        &mut fake_build(&mut Vec::new()),
    )
    .expect("the release passes with the fakes");

    let text = std::fs::read_to_string(&record).expect("record");
    let (calls, env): (Vec<&str>, Vec<&str>) =
        text.lines().partition(|l| !l.starts_with("signer-env "));
    let calls: Vec<Vec<String>> = calls
        .iter()
        .map(|l| l.split('\x1f').map(str::to_owned).collect())
        .collect();
    assert_eq!(calls, expected(&root, &ent));
    let signer_env = format!("signer-env {PASSWORD} unset");
    assert_eq!(
        env,
        [signer_env.as_str(); 2],
        "the password reached the signer by its environment"
    );

    let lines = sink.lock().expect("not poisoned").clone();
    // Control: the fakes did print the identity, so the redaction had something to hide.
    assert!(
        lines.iter().any(|l| l == "origin=<APPLE_SIGNING_IDENTITY>"),
        "{lines:#?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("using <TOKNILA_NOTARY_PROFILE> for <APPLE_SIGNING_IDENTITY>")),
        "{lines:#?}"
    );
    for l in &lines {
        for value in [ID, "TEAM123456", PROFILE, PASSWORD, &key.to_string_lossy()] {
            assert!(!l.contains(value), "a printed line shows a value: {l}");
        }
    }
}
