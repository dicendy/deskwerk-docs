//! Names written by builds from before the rename to Deskwerk Docs. They are only read, never
//! written: native `.json` documents, crash-recovery copies and DOCX files saved by an older build
//! keep opening as before. This is the only place the old app name may appear in the code.
use crate::Document;

/// The prefix of the app's own bookkeeping keys in [`Document::passthrough`] (old name).
/// Current builds write [`PASSTHROUGH_PREFIX`].
pub const OLD_PASSTHROUGH_PREFIX: &str = "wordcraft:";
/// The prefix of the app's own bookkeeping keys in [`Document::passthrough`].
pub const PASSTHROUGH_PREFIX: &str = "deskwerk-docs:";
/// The root element of an ODF embedded object's markup as older builds kept it (old name).
/// Current builds write [`crate::graphic::Embedded::ODF_OBJECT`].
pub const OLD_ODF_OBJECT: &str = "<wordcraft:odf-object";
/// The `a:ext` URI of a turned or flipped Drawing Canvas in DOCX files older builds wrote (old
/// name). Current builds write `urn:deskwerk-docs:canvas-xfrm`.
pub const OLD_CANVAS_SPIN_EXT: &str = "urn:wordcraft:canvas-xfrm";

impl Document {
    /// Rename the app's bookkeeping keys an older build left in `passthrough` (see
    /// [`OLD_PASSTHROUGH_PREFIX`]) to the current names. A current key of the same name wins.
    pub fn upgrade_legacy_names(&mut self) {
        let old: Vec<String> = self.passthrough.keys().filter(|k| k.starts_with(OLD_PASSTHROUGH_PREFIX)).cloned().collect();
        for key in old {
            let Some(bytes) = self.passthrough.remove(&key) else { continue };
            let rest = key.get(OLD_PASSTHROUGH_PREFIX.len()..).unwrap_or_default();
            self.passthrough.entry(format!("{PASSTHROUGH_PREFIX}{rest}")).or_insert(bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn old_bookkeeping_keys_get_the_current_prefix() {
        let mut d = Document::default();
        d.passthrough.insert(format!("{OLD_PASSTHROUGH_PREFIX}unmodelled.roots"), Arc::new(b"old".to_vec()));
        d.passthrough.insert(format!("{OLD_PASSTHROUGH_PREFIX}app.kept"), Arc::new(b"stale".to_vec()));
        d.passthrough.insert(format!("{PASSTHROUGH_PREFIX}app.kept"), Arc::new(b"new".to_vec()));
        d.passthrough.insert("word/custom.xml".into(), Arc::new(b"part".to_vec()));
        d.upgrade_legacy_names();
        let keys: Vec<&str> = d.passthrough.keys().map(String::as_str).collect();
        assert_eq!(keys, ["deskwerk-docs:app.kept", "deskwerk-docs:unmodelled.roots", "word/custom.xml"]);
        assert_eq!(d.passthrough["deskwerk-docs:unmodelled.roots"].as_slice(), b"old");
        assert_eq!(d.passthrough["deskwerk-docs:app.kept"].as_slice(), b"new");
    }
}
