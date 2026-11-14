//! The web font pack: open fonts the web build serves from its own origin, next to `index.html`
//! (`assets/webfonts`, copied there by Trunk), so a browser draws Word's fonts without fetching
//! anything from a third-party server.
//!
//! [`MANIFEST`] lists every file with its family, style, licence, source and SHA-256. A few
//! Regular faces are fetched at start ([`PackFile::eager`]); every other file only when a
//! document's text or the user's pick asks for its family ([`want`], [`take_wanted`]). The font
//! list shows Word's usual names ([`STAND_INS`]) and the popular Google fonts
//! ([`GOOGLE_FAMILIES`]); a document keeps the name it was given, and the stand-in only draws it.
//!
//! The desktop app builds the same files into the program and [`enable`]s the pack too (no
//! network there); the computer's installed fonts always win over a stand-in. Without [`enable`]
//! (the CLI, tests) the stand-in table plays no part and the font list stays the installed
//! families.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

/// `assets/webfonts/manifest.tsv`: one row per font file, tab-separated, with a header line.
pub const MANIFEST: &str = include_str!("../../../assets/webfonts/manifest.tsv");

/// The folder the web build serves the pack from, relative to `index.html`.
pub const WEB_DIR: &str = "webfonts";

/// How a stand-in relates to the font it draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Made to measure alike (the same advance widths): lines and pages break as with the font.
    Twin,
    /// The closest free design; widths differ, so lines can break elsewhere.
    Similar,
}

/// An Office font and the open font that draws it when the font itself isn't installed.
#[derive(Clone, Copy, Debug)]
pub struct StandIn {
    /// The name Word's font list shows and documents store.
    pub office: &'static str,
    /// The family that draws it: a pack family or a bundled one.
    pub family: &'static str,
    pub kind: Kind,
}

const fn twin(office: &'static str, family: &'static str) -> StandIn {
    StandIn { office, family, kind: Kind::Twin }
}

const fn similar(office: &'static str, family: &'static str) -> StandIn {
    StandIn { office, family, kind: Kind::Similar }
}

/// Word's usual fonts and what draws each one when it isn't installed:
///
/// | Office font | Drawn in | Kind |
/// |---|---|---|
/// | Calibri | Carlito | twin |
/// | Cambria | Caladea | twin |
/// | Arial, Helvetica | Liberation Sans | twin |
/// | Times New Roman | Liberation Serif | twin |
/// | Courier New | Liberation Mono | twin |
/// | Georgia | Gelasio | twin |
/// | Segoe UI | Selawik | twin |
/// | Calibri Light | Carlito | similar |
/// | Arial Narrow | PT Sans Narrow | similar |
/// | Verdana, Lucida Sans Unicode | DejaVu Sans | similar |
/// | Tahoma | PT Sans | similar |
/// | Trebuchet MS | Fira Sans | similar |
/// | Comic Sans MS | Comic Neue | similar |
/// | Impact | Anton | similar |
/// | Consolas | Inconsolata | similar |
/// | Lucida Console | DejaVu Sans Mono | similar |
/// | Garamond | EB Garamond | similar |
/// | Palatino Linotype, Book Antiqua | Lora | similar |
/// | Century Gothic | Didact Gothic | similar |
/// | Aptos, Aptos Display | Open Sans | similar |
/// | Candara | PT Sans | similar |
/// | Constantia | PT Serif | similar |
/// | Corbel | Source Sans 3 (bundled) | similar |
/// | Franklin Gothic Medium / Book / Demi / Heavy | Libre Franklin | similar |
///
/// The twins are also [`crate::word`]'s metric twins, tried first for a missing font.
pub const STAND_INS: &[StandIn] = &[
    twin("Arial", "Liberation Sans"),
    twin("Helvetica", "Liberation Sans"),
    twin("Times New Roman", "Liberation Serif"),
    twin("Courier New", "Liberation Mono"),
    twin("Calibri", "Carlito"),
    twin("Cambria", "Caladea"),
    twin("Georgia", "Gelasio"),
    twin("Segoe UI", "Selawik"),
    similar("Calibri Light", "Carlito"),
    similar("Arial Narrow", "PT Sans Narrow"),
    similar("Verdana", "DejaVu Sans"),
    similar("Lucida Sans Unicode", "DejaVu Sans"),
    similar("Tahoma", "PT Sans"),
    similar("Trebuchet MS", "Fira Sans"),
    similar("Comic Sans MS", "Comic Neue"),
    similar("Impact", "Anton"),
    similar("Consolas", "Inconsolata"),
    similar("Lucida Console", "DejaVu Sans Mono"),
    similar("Garamond", "EB Garamond"),
    similar("Palatino Linotype", "Lora"),
    similar("Book Antiqua", "Lora"),
    similar("Century Gothic", "Didact Gothic"),
    similar("Aptos", "Open Sans"),
    similar("Aptos Display", "Open Sans"),
    similar("Candara", "PT Sans"),
    similar("Constantia", "PT Serif"),
    similar("Corbel", "Source Sans 3"),
    similar("Franklin Gothic Medium", "Libre Franklin"),
    similar("Franklin Gothic Book", "Libre Franklin"),
    similar("Franklin Gothic Demi", "Libre Franklin"),
    similar("Franklin Gothic Heavy", "Libre Franklin"),
];

/// Popular Google fonts the pack carries under their own names (Source Sans 3 and Inter are
/// bundled in the app itself).
pub const GOOGLE_FAMILIES: &[&str] = &[
    "Roboto",
    "Open Sans",
    "Lato",
    "Montserrat",
    "Noto Sans",
    "Noto Serif",
    "PT Sans",
    "PT Serif",
    "Merriweather",
    "Lora",
    "Playfair Display",
    "Oswald",
    "Raleway",
    "Nunito",
    "Poppins",
    "EB Garamond",
    "Roboto Mono",
    "Roboto Slab",
    "Lexend",
    "Comic Neue",
];

/// One font file of the pack, a row of [`MANIFEST`].
#[derive(Clone, Debug)]
pub struct PackFile {
    /// File name in [`WEB_DIR`].
    pub file: &'static str,
    pub family: &'static str,
    /// `Regular`, `Bold`, `Italic` or `BoldItalic`.
    pub style: &'static str,
    /// Fetched at start rather than when a document asks for the family.
    pub eager: bool,
    pub license_spdx: &'static str,
    /// The licence text, relative to [`WEB_DIR`].
    pub license_file: &'static str,
    pub sha256: &'static str,
    pub bytes: u64,
    pub source_url: &'static str,
}

/// Columns of [`MANIFEST`], in order.
pub const MANIFEST_COLUMNS: &[&str] = &[
    "file",
    "family",
    "style",
    "group",
    "eager",
    "license_spdx",
    "license_file",
    "sha256",
    "bytes",
    "cyrillic",
    "latin_ext",
    "source_url",
    "copyright",
];

fn parse_row(line: &'static str) -> Option<PackFile> {
    let c: Vec<&'static str> = line.split('\t').collect();
    if c.len() != MANIFEST_COLUMNS.len() {
        return None;
    }
    let col = |i: usize| c.get(i).copied();
    Some(PackFile {
        file: col(0)?,
        family: col(1)?,
        style: col(2)?,
        eager: col(4)? == "yes",
        license_spdx: col(5)?,
        license_file: col(6)?,
        sha256: col(7)?,
        bytes: col(8)?.parse().ok()?,
        source_url: col(11)?,
    })
}

/// Every file of the pack, in manifest order.
pub fn files() -> &'static [PackFile] {
    static FILES: OnceLock<Vec<PackFile>> = OnceLock::new();
    FILES.get_or_init(|| MANIFEST.lines().skip(1).filter(|l| !l.trim().is_empty()).filter_map(parse_row).collect())
}

/// The files of `family` (a pack family name, any case).
pub fn files_of(family: &str) -> impl Iterator<Item = &'static PackFile> + '_ {
    files().iter().filter(move |f| f.family.eq_ignore_ascii_case(family.trim()))
}

/// The files fetched at start.
pub fn eager_files() -> impl Iterator<Item = &'static PackFile> {
    files().iter().filter(|f| f.eager)
}

/// Whether the pack has files for `family`.
pub fn has_family(family: &str) -> bool {
    files_of(family).next().is_some()
}

/// The stand-in for the Office font `family` (any case; a charset alias such as
/// `Times New Roman Cyr` counts as the font itself).
pub fn stand_in(family: &str) -> Option<&'static StandIn> {
    let base = crate::word::charset_base_family(family.trim());
    STAND_INS.iter().find(|s| s.office.eq_ignore_ascii_case(base))
}

/// Whether `substitute` is the font [`STAND_INS`] gives `family` (or, for a fallback list, one of
/// its entries).
pub fn stands_in(family: &str, substitute: &str) -> bool {
    family.split(';').map(str::trim).filter(|f| !f.is_empty()).any(|f| stand_in(f).is_some_and(|s| s.family.eq_ignore_ascii_case(substitute.trim())))
}

/// The pack family that draws `family`: the family itself when the pack carries it, else its
/// stand-in's when the pack carries that.
pub fn pack_family_for(family: &str) -> Option<&'static str> {
    let f = family.trim();
    if let Some(p) = files().iter().find(|p| p.family.eq_ignore_ascii_case(f)) {
        return Some(p.family);
    }
    stand_in(f).map(|s| s.family).filter(|s| has_family(s))
}

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Turn the pack on (the web build, at start): the font list shows its names and missing fonts
/// ask for their pack files.
pub fn enable() {
    ENABLED.store(true, Ordering::Release);
}

/// Whether [`enable`] was called.
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

#[derive(Default)]
struct Requests {
    /// Families asked for and not yet taken.
    wanted: BTreeSet<&'static str>,
    /// Families asked for once already: never asked again.
    asked: BTreeSet<&'static str>,
}

fn requests() -> &'static Mutex<Requests> {
    static R: OnceLock<Mutex<Requests>> = OnceLock::new();
    R.get_or_init(Mutex::default)
}

/// A document's text or the user's pick needs `family` (a family or a `;` fallback list) and it
/// isn't installed: when the pack is on, ask for the pack family that draws it. Each family is
/// asked for once.
pub fn want(family: &str) {
    if !enabled() {
        return;
    }
    // A pack family the computer has already (Liberation Serif on Linux) needs no files.
    let db = crate::FontDb::global();
    let needed: Vec<&'static str> =
        family.split(';').map(str::trim).filter(|f| !f.is_empty()).take(16).filter_map(pack_family_for).filter(|p| !db.has_family(p)).collect();
    let mut r = requests().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    for p in needed {
        if !r.asked.contains(p) {
            r.wanted.insert(p);
        }
    }
}

/// The pack families asked for since the last call ([`want`]); the web shell fetches their files.
pub fn take_wanted() -> Vec<&'static str> {
    let mut r = requests().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let wanted = std::mem::take(&mut r.wanted);
    r.asked.extend(wanted.iter().copied());
    wanted.into_iter().collect()
}

/// Add fetched pack files to the font database, each under its manifest family, in one change:
/// the layout notices the new generation once. Returns the faces added.
pub fn add_files(files: Vec<(&PackFile, Vec<u8>)>) -> usize {
    crate::FontDb::global().add_document_fonts(files.into_iter().map(|(f, bytes)| (bytes, f.family.to_string())).collect())
}

/// The names the font list adds while the pack is on: Word's usual fonts and the Google fonts.
pub fn list_families() -> Vec<&'static str> {
    if !enabled() {
        return Vec::new();
    }
    let mut v: Vec<&'static str> = STAND_INS.iter().map(|s| s.office).chain(GOOGLE_FAMILIES.iter().copied()).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// The family a font list entry is previewed in when `family` itself isn't installed: its
/// stand-in.
pub fn preview_family(family: &str) -> Option<&'static str> {
    stand_in(family).map(|s| s.family)
}

/// Whether `family` is shown in a stand-in while the pack is on: an Office font from
/// [`STAND_INS`] that isn't installed. The font list and the font box mark it.
pub fn drawn_by_stand_in(family: &str) -> Option<&'static StandIn> {
    if !enabled() {
        return None;
    }
    stand_in(family).filter(|_| !crate::FontDb::global().has_family(family.trim()))
}

#[cfg(test)]
mod tests;
