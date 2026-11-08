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

#[test]
fn ships_go_to_a_folder_without_a_space_and_not_to_the_legacy_home() {
    let user = Path::new("/r/Users/me");
    let home = Path::new("/r/Users/me/Library/Application Support/toknila");
    let root = ships_root(None, Some(user), home);
    assert_eq!(root, user.join(".toknila-ships"));
    assert!(!root.to_string_lossy().contains(' '));
    assert_ne!(root, user.join(".toknila"));
    assert!(!root.starts_with(user.join(".toknila")));
}

#[test]
fn the_ships_override_wins_and_a_missing_user_home_falls_back_to_the_data_home() {
    let home = Path::new("/h");
    let over = PathBuf::from("/x/ships");
    assert_eq!(
        ships_root(Some(over.clone()), Some(Path::new("/u")), home),
        over
    );
    assert_eq!(ships_root(None, None, home), home.join("ships"));
}

#[test]
fn an_explicit_home_keeps_its_ships_inside_it() {
    assert_eq!(Paths::at("/t/home").ships(), Path::new("/t/home/ships"));
}

#[test]
fn the_default_home_passed_explicitly_gets_the_default_ships() {
    let user = Path::new("/u");
    let def = Path::new("/u/Library/Application Support/toknila");
    assert_eq!(
        explicit_ships(None, def, Some(def), Some(user)),
        user.join(".toknila-ships")
    );
    let over = PathBuf::from("/o");
    assert_eq!(
        explicit_ships(Some(over.clone()), def, Some(def), Some(user)),
        over
    );
}

#[test]
fn another_explicit_home_keeps_its_ships_inside_it() {
    let user = Path::new("/u");
    let def = Path::new("/u/.toknila");
    let tmp = Path::new("/t/home");
    assert_eq!(
        explicit_ships(None, tmp, Some(def), Some(user)),
        tmp.join("ships")
    );
    assert_eq!(explicit_ships(None, tmp, None, None), tmp.join("ships"));
}
