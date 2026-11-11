use super::*;
use toknila_core::design::DesignKind;

pub(crate) const PNG: &[u8] = &[
    0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, b'I', b'H', b'D', b'R', 0, 0, 0,
    1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1F, 0x15, 0xC4, 0x89,
];

fn store(dir: &Path) -> Designs {
    Designs::new(dir, ProjectId::new())
}

fn frame() -> FigmaRef {
    design::parse_figma("https://www.figma.com/design/AbC123/App?node-id=1-2").expect("frame")
}

#[test]
fn a_kept_file_survives_a_new_store_over_the_same_folder() {
    let dir = tempfile::tempdir().expect("dir");
    let a = store(dir.path());
    let made = a.add_file("home.png", PNG, Timestamp(5)).expect("add");
    assert_eq!(
        (made.kind, made.thumb, made.size),
        (DesignKind::Image, true, PNG.len() as u64)
    );
    // A process restart is a new value over the same folder.
    let b = Designs { ..a.clone() };
    let listed = b.list().expect("list");
    assert_eq!(listed, vec![made.clone()]);
    let (bytes, mime) = b.read_file(made.id).expect("file");
    assert_eq!((bytes.as_slice(), mime.as_str()), (PNG, "image/png"));
    // The file is under the data home, named by id, not by the person's name.
    assert!(a.dir.join(format!("{}.png", made.id)).is_file());
}

#[test]
fn the_bytes_decide_not_the_name() {
    let dir = tempfile::tempdir().expect("dir");
    let s = store(dir.path());
    assert!(matches!(
        s.add_file("evil.png", b"<html>", Timestamp(0)),
        Err(DesignError::Invalid(_))
    ));
    let pdf = s
        .add_file("flow.png", b"%PDF-1.4 x", Timestamp(0))
        .expect("pdf");
    assert_eq!(
        (pdf.kind, pdf.thumb, pdf.mime.as_deref()),
        (DesignKind::Pdf, false, Some("application/pdf"))
    );
    // The name never reaches the disk path.
    let evil = s
        .add_file("../../x.png", PNG, Timestamp(0))
        .expect("name cleaned");
    assert_eq!(evil.name, "x.png");
    assert!(!dir.path().join("x.png").exists());
}

#[test]
fn rename_remove_and_names() {
    let dir = tempfile::tempdir().expect("dir");
    let s = store(dir.path());
    let a = s.add_file("a.png", PNG, Timestamp(1)).expect("a");
    let link = s
        .add_figma("login frame", frame(), Timestamp(2))
        .expect("link");
    assert_eq!(s.names(), ["a.png", "login frame"]);
    s.rename(a.id, "b.png").expect("rename");
    assert_eq!(s.names(), ["b.png", "login frame"]);
    assert!(s.rename(a.id, " ").is_err());
    s.remove(a.id).expect("remove");
    assert!(!s.dir.join(format!("{}.png", a.id)).exists());
    assert!(matches!(s.remove(a.id), Err(DesignError::NotFound(_))));
    assert_eq!(s.list().expect("list").len(), 1);
    s.remove(link.id).expect("remove link");
}

#[test]
fn a_link_without_an_image_has_the_hint_for_its_state() {
    let dir = tempfile::tempdir().expect("dir");
    let s = store(dir.path());
    s.add_figma("f", frame(), Timestamp(0)).expect("link");
    let hint = |s: &Designs| s.list().expect("list")[0].hint.clone().unwrap_or_default();
    assert!(hint(&s).contains("Export the frame"), "{}", hint(&s));
    s.set_token("figd_secret-1").expect("token");
    assert!(hint(&s).contains("token is saved"), "{}", hint(&s));
}

#[test]
fn the_token_is_a_private_file_under_secrets_and_nowhere_else() {
    let dir = tempfile::tempdir().expect("dir");
    let s = store(dir.path());
    assert!(!s.has_token());
    s.set_token(" figd_secret-1 ").expect("token");
    assert_eq!(s.token().as_deref(), Some("figd_secret-1"));
    assert!(s.secret.starts_with(dir.path().join("secrets")));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&s.secret).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    s.add_figma("f", frame(), Timestamp(0)).expect("link");
    let index = fs::read_to_string(s.index()).expect("index");
    assert!(!index.contains("figd_secret"), "{index}");
    assert!(s.set_token("bad token").is_err());
    s.clear_token().expect("clear");
    assert!(!s.has_token());
    s.clear_token().expect("clear twice");
}

#[test]
fn delete_all_takes_designs_and_token() {
    let dir = tempfile::tempdir().expect("dir");
    let s = store(dir.path());
    s.add_file("a.png", PNG, Timestamp(0)).expect("a");
    s.set_token("figd_x").expect("token");
    s.delete_all();
    assert!(s.list().expect("list").is_empty());
    assert!(!s.has_token());
}
