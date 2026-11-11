use super::*;

#[test]
fn sniff_tells_the_four_formats_from_their_bytes() {
    let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
    assert_eq!(sniff(&png).map(|s| s.mime), Some("image/png"));
    assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]).map(|s| s.ext), Some("jpg"));
    assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 ").map(|s| s.ext), Some("webp"));
    let pdf = sniff(b"%PDF-1.7\n").expect("pdf");
    assert_eq!((pdf.kind, pdf.mime), (DesignKind::Pdf, "application/pdf"));
    // Controls: text, an empty file and a RIFF that is not webp are none of them.
    assert_eq!(sniff(b"<svg xmlns=''/>"), None);
    assert_eq!(sniff(b""), None);
    assert_eq!(sniff(b"RIFF\0\0\0\0WAVEfmt "), None);
}

#[test]
fn figma_link_gives_key_and_node() {
    let r =
        parse_figma("https://www.figma.com/design/AbC123/My-app?node-id=12-34&t=zz").expect("link");
    assert_eq!(
        (r.file_key.as_str(), r.node_id.as_str()),
        ("AbC123", "12:34")
    );
    let old = parse_figma("https://figma.com/file/Key9/x?type=design&node-id=1%3A2").expect("old");
    assert_eq!(old.node_id, "1:2");
    let branch =
        parse_figma("https://www.figma.com/design/Main1/branch/Br2/n?node-id=5-6").expect("branch");
    assert_eq!(branch.file_key, "Br2");
}

#[test]
fn figma_link_without_a_frame_or_from_elsewhere_is_refused() {
    assert!(parse_figma("https://www.figma.com/design/AbC123/My-app").is_err());
    assert!(parse_figma("https://evil.example/design/AbC123/x?node-id=1-2").is_err());
    assert!(parse_figma("https://notfigma.com/design/AbC123/x?node-id=1-2").is_err());
    assert!(parse_figma("https://www.figma.com/design/AbC123/x?node-id=a-b").is_err());
    assert!(parse_figma("ftp://www.figma.com/design/AbC123/x?node-id=1-2").is_err());
}

#[test]
fn names_lose_their_path() {
    assert_eq!(clean_name("C:\\x\\y.png").as_deref(), Ok("y.png"));
    assert_eq!(clean_name("../../etc/passwd").as_deref(), Ok("passwd"));
    assert!(clean_name("  ").is_err());
    assert!(clean_name("..").is_err());
}

#[test]
fn a_token_with_a_quote_or_space_is_refused() {
    assert_eq!(clean_token(" figd_abc-123 ").as_deref(), Ok("figd_abc-123"));
    assert!(clean_token("a b").is_err());
    assert!(clean_token("a\"b").is_err());
    assert!(clean_token("").is_err());
}
