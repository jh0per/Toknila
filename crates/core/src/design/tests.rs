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

#[test]
fn a_region_is_percent_of_the_image_and_stays_inside_it() {
    let r = |x, y, w, h| Region { x, y, w, h };
    assert_eq!(r(0, 0, 100, 100).check(), Ok(()));
    assert_eq!(r(20, 10, 40, 30).check(), Ok(()));
    // Controls: empty, out of the image, and past the edge are each refused.
    assert!(r(20, 10, 0, 30).check().is_err());
    assert!(r(20, 10, 40, 0).check().is_err());
    assert!(r(101, 0, 1, 1).check().is_err());
    assert!(r(70, 0, 40, 10).check().is_err());
    assert!(r(0, 95, 10, 10).check().is_err());
}

#[test]
fn design_refs_are_checked_and_have_no_double() {
    let id = DesignId::new();
    let one = DesignRef {
        design: id,
        region: Some(Region {
            x: 1,
            y: 2,
            w: 3,
            h: 4,
        }),
    };
    assert_eq!(check_refs(std::slice::from_ref(&one)), Ok(()));
    let bad = DesignRef {
        design: id,
        region: Some(Region {
            x: 99,
            y: 2,
            w: 3,
            h: 4,
        }),
    };
    assert!(check_refs(&[bad]).is_err());
    // The same design twice (without regions) is a mistake; with different regions it is not.
    let plain = DesignRef {
        design: id,
        region: None,
    };
    assert!(check_refs(&[plain.clone(), plain.clone()]).is_err());
    assert_eq!(check_refs(&[plain, one]), Ok(()));
    let json = serde_json::to_string(&DesignRef {
        design: id,
        region: None,
    })
    .expect("json");
    assert_eq!(json, format!("{{\"design\":\"{id}\"}}"));
}
