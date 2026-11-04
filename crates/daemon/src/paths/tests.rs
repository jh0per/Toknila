use super::*;

fn dirs_in(root: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    (
        Some(root.join("Users/me")),
        Some(root.join("Users/me/Library/Application Support")),
    )
}

#[test]
fn each_platform_has_its_place() {
    let root = Path::new("/r");
    let (home, data) = dirs_in(root);
    let mac = layout("macos", home.clone(), data.clone()).expect("macos");
    assert_eq!(
        mac.home,
        root.join("Users/me/Library/Application Support/toknila")
    );
    assert_eq!(mac.legacy, Some(root.join("Users/me/.toknila")));
    let linux = layout("linux", home.clone(), data.clone()).expect("linux");
    assert_eq!(
        (linux.home, linux.legacy),
        (root.join("Users/me/.toknila"), None)
    );
    let win = layout("windows", home, data).expect("windows");
    assert_eq!(
        win.home,
        root.join("Users/me/Library/Application Support/toknila")
    );
    assert_eq!(layout("macos", None, None).map(|l| l.home), None);
}

fn mac_layout(root: &Path) -> Layout {
    let (home, data) = dirs_in(root);
    layout("macos", home, data).expect("layout")
}

#[test]
fn legacy_home_moves_once_with_its_files() {
    let tmp = tempfile::tempdir().expect("tmp");
    let l = mac_layout(tmp.path());
    let legacy = l.legacy.clone().expect("legacy");
    std::fs::create_dir_all(legacy.join("logs")).expect("mkdir");
    std::fs::write(legacy.join("toknila.db"), b"db").expect("db");
    assert_eq!(settle_home(&l).expect("move"), l.home);
    assert_eq!(
        std::fs::read(l.home.join("toknila.db")).expect("moved db"),
        b"db"
    );
    assert!(l.home.join("logs").is_dir());
    assert!(!legacy.exists());
    // Second start: nothing left to move.
    assert_eq!(settle_home(&l).expect("again"), l.home);
}

#[test]
fn a_home_in_use_or_already_there_is_not_touched() {
    let tmp = tempfile::tempdir().expect("tmp");
    let l = mac_layout(tmp.path());
    let legacy = l.legacy.clone().expect("legacy");
    std::fs::create_dir_all(&legacy).expect("mkdir");
    std::fs::write(legacy.join("daemon.json"), b"{}").expect("daemon.json");
    assert_eq!(settle_home(&l).expect("busy"), legacy);
    assert!(legacy.join("daemon.json").exists());

    std::fs::remove_file(legacy.join("daemon.json")).expect("rm");
    std::fs::create_dir_all(&l.home).expect("new home");
    assert_eq!(settle_home(&l).expect("both"), l.home);
    assert!(
        legacy.exists(),
        "an existing new home wins; the old one is left alone"
    );
}

#[test]
fn fresh_install_uses_the_platform_place() {
    let tmp = tempfile::tempdir().expect("tmp");
    let l = mac_layout(tmp.path());
    assert_eq!(settle_home(&l).expect("fresh"), l.home);
    assert!(
        !l.home.exists(),
        "created by the daemon on start, not by resolving"
    );
}
