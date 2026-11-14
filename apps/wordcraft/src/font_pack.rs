//! The font pack (`wordcraft_fonts::pack`) built into the desktop app: the same open fonts the
//! web build serves from `webfonts/`, so Word's fonts and the Google fonts are there with no
//! network. Fonts installed on the computer win: a pack family is added only when a document's
//! text or the user's pick asks for a family the computer doesn't have.

use wordcraft_fonts::pack::{self, PackFile};

/// `(file name, bytes)` for every file in `assets/webfonts/manifest.tsv` (written by `build.rs`).
static FILES: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/font_pack.rs"));

fn bytes(f: &PackFile) -> Option<&'static [u8]> {
    FILES.iter().find(|(name, _)| *name == f.file).map(|(_, b)| *b)
}

/// Add `files` to the font database in one change (one relayout).
fn add(files: impl Iterator<Item = &'static PackFile>) {
    let got: Vec<(&PackFile, Vec<u8>)> = files.filter_map(|f| bytes(f).map(|b| (f, b.to_vec()))).collect();
    if !got.is_empty() {
        pack::add_files(got);
    }
}

/// Turn the pack on: the font list shows Word's fonts and the Google fonts, and a missing font
/// asks for its stand-in ([`poll`] adds it).
pub(crate) fn start() {
    pack::enable();
}

/// Add the pack families the layout asked for since the last frame ([`pack::take_wanted`]), and
/// draw again with them.
pub(crate) fn poll(ctx: &egui::Context) {
    let wanted = pack::take_wanted();
    for family in &wanted {
        add(pack::files_of(family));
    }
    if !wanted.is_empty() {
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pack_file_is_built_in() {
        assert_eq!(FILES.len(), pack::files().len());
        for f in pack::files() {
            assert_eq!(bytes(f).map(<[u8]>::len), Some(usize::try_from(f.bytes).unwrap_or(0)), "{}", f.file);
        }
    }
}
