use std::path::PathBuf;

use super::*;

fn pack_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/webfonts")
}

/// The licences ATTRIBUTION.md and AGENTS.md allow for fonts.
const ALLOWED_LICENCES: &[&str] = &["OFL-1.1", "Apache-2.0", "Bitstream-Vera AND LicenseRef-Arev-Fonts"];

/// SHA-256 (FIPS 180-4), enough to check the manifest without another dependency.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be,
        0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa,
        0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85,
        0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f,
        0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    for block in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (slot, word) in w.iter_mut().zip(block.chunks_exact(4)) {
            *slot = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for (k, wi) in K.iter().zip(w) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(*k).wrapping_add(wi);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[test]
fn sha256_matches_known_vectors() {
    assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn the_manifest_lists_every_pack_file_with_its_licence() {
    let mut header = MANIFEST.lines().next().unwrap_or_default().split('\t');
    assert!(MANIFEST_COLUMNS.iter().all(|c| header.next() == Some(*c)), "manifest header");
    let rows = MANIFEST.lines().skip(1).filter(|l| !l.trim().is_empty()).count();
    assert_eq!(files().len(), rows, "every manifest row parses");
    let dir = pack_dir();
    let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".ttf") || n.ends_with(".otf"))
        .collect();
    on_disk.sort();
    let mut listed: Vec<String> = files().iter().map(|f| f.file.to_string()).collect();
    listed.sort();
    assert_eq!(on_disk, listed, "the font files in assets/webfonts are the manifest's");
    for f in files() {
        let bytes = std::fs::read(dir.join(f.file)).unwrap();
        assert_eq!(bytes.len() as u64, f.bytes, "{}", f.file);
        assert_eq!(sha256_hex(&bytes), f.sha256, "{}", f.file);
        assert!(ALLOWED_LICENCES.contains(&f.license_spdx), "{}: {}", f.file, f.license_spdx);
        let licence = std::fs::read_to_string(dir.join(f.license_file)).unwrap();
        assert!(licence.len() > 500, "{}: {}", f.file, f.license_file);
        if f.license_spdx == "OFL-1.1" {
            assert!(licence.contains("SIL OPEN FONT LICENSE"), "{}", f.license_file);
        }
        assert!(["Regular", "Bold", "Italic", "BoldItalic"].contains(&f.style), "{}", f.file);
        assert!(f.source_url.starts_with("https://"), "{}", f.file);
        // Static fonts only: a variable font would show one instance for every weight.
        let face = skrifa::FontRef::new(&bytes).unwrap();
        use skrifa::raw::TableProvider as _;
        assert!(face.fvar().is_err(), "{} is a variable font", f.file);
    }
}

#[test]
fn every_licence_text_in_the_pack_is_used() {
    let dir = pack_dir().join("licenses");
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let name = format!("licenses/{}", e.file_name().to_string_lossy());
        assert!(files().iter().any(|f| f.license_file == name), "{name} isn't any file's licence");
    }
}

#[test]
fn every_stand_in_and_google_family_is_in_the_pack() {
    let bundled = ["Source Sans 3", "Source Serif 4", "Inter", "JetBrains Mono"];
    for s in STAND_INS {
        assert!(bundled.contains(&s.family) || files_of(s.family).any(|f| f.style == "Regular"), "{} → {}", s.office, s.family);
        if s.kind == Kind::Twin {
            assert!(crate::word::metric_compatible(s.office, s.family), "{} ↔ {}", s.office, s.family);
        }
    }
    for g in GOOGLE_FAMILIES {
        for style in ["Regular", "Bold"] {
            assert!(files_of(g).any(|f| f.style == style), "{g} {style}");
        }
    }
}

#[test]
fn the_eager_set_stays_small() {
    let eager: Vec<_> = eager_files().collect();
    assert!(!eager.is_empty());
    assert!(eager.iter().all(|f| f.style == "Regular"));
    let bytes: u64 = eager.iter().map(|f| f.bytes).sum();
    assert!(bytes < 2 << 20, "{bytes} bytes fetched at start");
}

#[test]
fn office_names_map_to_their_stand_ins() {
    let drawn = |f: &str| stand_in(f).map(|s| (s.family, s.kind));
    assert_eq!(drawn("Times New Roman"), Some(("Liberation Serif", Kind::Twin)));
    assert_eq!(drawn("times new roman cyr"), Some(("Liberation Serif", Kind::Twin)));
    assert_eq!(drawn("Arial"), Some(("Liberation Sans", Kind::Twin)));
    assert_eq!(drawn("Courier New"), Some(("Liberation Mono", Kind::Twin)));
    assert_eq!(drawn("Calibri"), Some(("Carlito", Kind::Twin)));
    assert_eq!(drawn("Cambria"), Some(("Caladea", Kind::Twin)));
    assert_eq!(drawn("Georgia"), Some(("Gelasio", Kind::Twin)));
    assert_eq!(drawn("Segoe UI"), Some(("Selawik", Kind::Twin)));
    assert_eq!(drawn("Verdana"), Some(("DejaVu Sans", Kind::Similar)));
    assert_eq!(drawn("Trebuchet MS"), Some(("Fira Sans", Kind::Similar)));
    assert_eq!(drawn("Comic Sans MS"), Some(("Comic Neue", Kind::Similar)));
    assert_eq!(drawn("Consolas"), Some(("Inconsolata", Kind::Similar)));
    assert_eq!(drawn("Garamond"), Some(("EB Garamond", Kind::Similar)));
    assert_eq!(drawn("Book Antiqua"), Some(("Lora", Kind::Similar)));
    assert_eq!(drawn("Aptos"), Some(("Open Sans", Kind::Similar)));
    assert_eq!(drawn("Roboto"), None);
    assert_eq!(drawn("No Such Font"), None);
    assert!(stands_in("Verdana", "DejaVu Sans"));
    assert!(stands_in("Foo;Impact", "Anton"));
    assert!(!stands_in("Verdana", "Carlito"));
    assert_eq!(pack_family_for("Verdana"), Some("DejaVu Sans"));
    assert_eq!(pack_family_for("roboto"), Some("Roboto"));
    assert_eq!(pack_family_for("Corbel"), None, "Source Sans 3 is bundled");
    assert_eq!(pack_family_for("No Such Font"), None);
}

#[test]
fn the_list_names_appear_only_with_the_pack() {
    // Each test runs in its own process under nextest; enabling here touches no other test.
    if enabled() {
        return;
    }
    assert!(list_families().is_empty());
    assert!(drawn_by_stand_in("Times New Roman").is_none());
    want("Times New Roman");
    assert!(take_wanted().is_empty(), "nothing is fetched without the pack");
    enable();
    let names = list_families();
    for n in ["Times New Roman", "Calibri", "Verdana", "Franklin Gothic Medium", "Roboto", "EB Garamond"] {
        assert!(names.contains(&n), "{n}");
    }
    want("Times New Roman;Verdana");
    want("Roboto");
    want("Corbel");
    assert_eq!(take_wanted(), ["DejaVu Sans", "Liberation Serif", "Roboto"]);
    want("Roboto");
    assert!(take_wanted().is_empty(), "each family is asked for once");
}

#[test]
fn a_missing_office_font_is_drawn_in_its_pack_stand_in() {
    if enabled() {
        return;
    }
    enable();
    let db = crate::FontDb::global();
    let dir = pack_dir();
    for family in ["Liberation Serif", "Fira Sans", "Comic Neue"] {
        let added = add_files(files_of(family).map(|f| (f, std::fs::read(dir.join(f.file)).unwrap())).collect());
        assert!(added > 0 || db.has_family(family), "{family}");
        assert!(db.has_family(family), "{family}");
    }
    for (office, drawn) in [("Times New Roman", "Liberation Serif"), ("Trebuchet MS", "Fira Sans"), ("Comic Sans MS", "Comic Neue")] {
        if db.has_family(office) {
            continue; // the real font wins
        }
        let r = crate::word::resolve(office, false, false);
        assert!(r.substituted, "{office}");
        assert_eq!(r.face.family, drawn, "{office}");
        let b = crate::word::resolve(office, true, true);
        assert_eq!(b.face.family, drawn, "{office} bold italic");
        assert!(!b.synth_bold && !b.synth_italic, "{office}: the pack has the bold italic face");
        assert!(drawn_by_stand_in(office).is_some(), "{office}");
    }
    // A family the pack carries under its own name is no longer substituted once added.
    assert!(!crate::word::resolve("Comic Neue", false, false).substituted);
    assert!(drawn_by_stand_in("Comic Neue").is_none());
}
