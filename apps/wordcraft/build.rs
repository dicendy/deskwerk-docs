//! Every target: list the font pack (`assets/webfonts/manifest.tsv`) for `src/font_pack.rs`, which
//! builds its files into the program.
//!
//! Windows only: embed the app icon and version info (VERSIONINFO) into `deskwerk-docs.exe`, so it
//! shows in Explorer, the taskbar, the Start menu and Alt-Tab.
//!
//! On every other target this does nothing. A missing resource compiler is a warning, so a
//! cross-compile from macOS or Linux still links, unless `WORDCRAFT_REQUIRE_WINRES=1` turns it
//! into an error (for release builds).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/wordcraft.ico");
    println!("cargo:rerun-if-env-changed=WORDCRAFT_REQUIRE_WINRES");
    font_pack();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/app-icon/wordcraft.ico")
        .set("ProductName", "Deskwerk Docs")
        .set("FileDescription", "Deskwerk Docs word processor")
        .set("CompanyName", "Deskwerk")
        .set("LegalCopyright", "Copyright (c) the Deskwerk Docs contributors and the WordCraft contributors. MIT OR Apache-2.0.")
        .set("OriginalFilename", "deskwerk-docs.exe")
        .set("InternalName", "deskwerk-docs");
    if let Err(e) = res.compile() {
        if std::env::var_os("WORDCRAFT_REQUIRE_WINRES").is_some() {
            eprintln!("embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=deskwerk-docs.exe built without icon/version resources: {e}");
    }
}

/// Writes `$OUT_DIR/font_pack.rs`: a slice of `(file name, include_bytes!(file))` for every file in
/// the font pack's manifest, so the desktop app carries the same fonts the web build serves.
fn font_pack() {
    const MANIFEST: &str = "../../assets/webfonts/manifest.tsv";
    println!("cargo:rerun-if-changed={MANIFEST}");
    let manifest = match std::fs::read_to_string(MANIFEST) {
        Ok(m) => m,
        Err(e) => {
            println!("cargo::error={MANIFEST}: {e}");
            return;
        }
    };
    let mut out = String::from("&[\n");
    for file in manifest.lines().skip(1).filter_map(|l| l.split('\t').next()).filter(|f| !f.trim().is_empty()) {
        println!("cargo:rerun-if-changed=../../assets/webfonts/{file}");
        out.push_str(&format!(
            "    ({file:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../assets/webfonts/\", {file:?})) as &[u8]),\n"
        ));
    }
    out.push(']');
    let Some(dir) = std::env::var_os("OUT_DIR") else {
        println!("cargo::error=OUT_DIR is not set");
        return;
    };
    if let Err(e) = std::fs::write(std::path::Path::new(&dir).join("font_pack.rs"), out) {
        println!("cargo::error=font_pack.rs: {e}");
    }
}
