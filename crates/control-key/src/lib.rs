//! The control channel's key file, one rule for the app (which writes it) and the MCP bridge
//! (which reads it): `<settings>/control-key.<instance>`. See `docs/control-protocol.md`, Keys.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::{Path, PathBuf};

/// The app's settings folder (preferences, logs, key files): macOS
/// `~/Library/Application Support/Deskwerk Docs`, Windows `%APPDATA%\Deskwerk Docs`, otherwise
/// `$XDG_CONFIG_HOME/deskwerk-docs` or `~/.config/deskwerk-docs`.
/// `DESKWERK_DOCS_PROFILE_DIR` overrides the entire folder for isolated runs.
pub fn settings_dir() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DESKWERK_DOCS_PROFILE_DIR").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/Deskwerk Docs"))
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Deskwerk Docs"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|c| c.join("deskwerk-docs"))
    }
}

/// The settings folder's name before the rename to Deskwerk Docs (macOS and Windows, then other
/// systems). Only [`migrate_old_settings`] reads it; nothing is ever written there.
const OLD_SETTINGS_NAMES: (&str, &str) = ("WordCraft", "wordcraft");

/// The folder older builds kept their settings in, beside [`settings_dir`]'s.
fn old_settings_dir(new: &Path) -> Option<PathBuf> {
    let name = if cfg!(any(target_os = "macos", windows)) { OLD_SETTINGS_NAMES.0 } else { OLD_SETTINGS_NAMES.1 };
    Some(new.parent()?.join(name))
}

/// First start after the rename: when the settings folder doesn't exist yet but the one older
/// builds used does, copy that one (preferences, dictionary, templates, fonts, recovery copies)
/// to the new place. The old folder is left as it was. Control-key files are not copied: they
/// belong to a running instance. Nothing happens with `DESKWERK_DOCS_PROFILE_DIR` set. Returns
/// the folder copied from, if any.
pub fn migrate_old_settings() -> Result<Option<PathBuf>, String> {
    if std::env::var_os("DESKWERK_DOCS_PROFILE_DIR").is_some_and(|p| !p.is_empty()) {
        return Ok(None);
    }
    let Some(new) = settings_dir() else { return Ok(None) };
    let Some(old) = old_settings_dir(&new) else { return Ok(None) };
    migrate_from(&old, &new)
}

fn migrate_from(old: &Path, new: &Path) -> Result<Option<PathBuf>, String> {
    if std::fs::symlink_metadata(new).is_ok() || !std::fs::symlink_metadata(old).is_ok_and(|m| m.is_dir()) {
        return Ok(None);
    }
    // Copied beside the new folder first, then renamed: an interrupted copy is never taken for
    // the settings folder.
    let mut partial = new.as_os_str().to_owned();
    partial.push(".partial");
    let partial = PathBuf::from(partial);
    let _ = std::fs::remove_dir_all(&partial);
    let copied = copy_tree(old, &partial, 0).and_then(|()| std::fs::rename(&partial, new));
    if let Err(e) = copied {
        let _ = std::fs::remove_dir_all(&partial);
        return Err(format!("copying {} to {}: {e}", old.display(), new.display()));
    }
    Ok(Some(old.to_path_buf()))
}

/// Copy the folder `from` to `to` (which must not exist): regular files and folders only, no
/// symbolic links, at most 16 levels deep.
fn copy_tree(from: &Path, to: &Path, depth: usize) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    if depth >= 16 {
        return Ok(());
    }
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name();
        if kind.is_dir() {
            copy_tree(&entry.path(), &to.join(&name), depth + 1)?;
        } else if kind.is_file() && !name.to_str().is_some_and(|n| n.starts_with("control-key.")) {
            std::fs::copy(entry.path(), to.join(&name))?;
        }
    }
    Ok(())
}

/// The `instance-id=` value of a `/.flatpak-info` file, only when it is `[A-Za-z0-9_-]+` (it
/// becomes part of a file name).
pub fn parse_instance_id(info: &str) -> Option<String> {
    let v = info.lines().find_map(|l| l.strip_prefix("instance-id="))?.trim();
    (!v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')).then(|| v.to_string())
}

/// The Flatpak instance id, or `None` outside Flatpak.
pub fn instance_id() -> Option<String> {
    parse_instance_id(&std::fs::read_to_string("/.flatpak-info").ok()?)
}

/// `<settings>/control-key.<instance>`, with `<instance>` the Flatpak instance id or
/// `local-<port>`.
pub fn key_path(settings: &Path, port: u16) -> PathBuf {
    key_path_for(settings, instance_id().as_deref(), port)
}

fn key_path_for(settings: &Path, instance: Option<&str>, port: u16) -> PathBuf {
    match instance {
        Some(id) => settings.join(format!("control-key.{id}")),
        None => settings.join(format!("control-key.local-{port}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn instance_id_only_safe_values() {
        assert_eq!(parse_instance_id("[Instance]\ninstance-id=abc_123-X\n").as_deref(), Some("abc_123-X"));
        assert_eq!(parse_instance_id("instance-id= 42 \n").as_deref(), Some("42"));
        // It ends up in a file name.
        assert_eq!(parse_instance_id("instance-id=../etc\n"), None);
        assert_eq!(parse_instance_id("instance-id=a/b\n"), None);
        assert_eq!(parse_instance_id("instance-id=a b\n"), None);
        assert_eq!(parse_instance_id("instance-id=..\n"), None);
        assert_eq!(parse_instance_id("instance-id=\n"), None);
        assert_eq!(parse_instance_id("app=eu.deskwerk.Docs\n"), None);
    }

    #[test]
    fn key_file_per_instance() {
        let dir = Path::new("/settings/deskwerk-docs");
        assert_eq!(key_path_for(dir, Some("1234"), 7981), dir.join("control-key.1234"));
        assert_eq!(key_path_for(dir, None, 7981), dir.join("control-key.local-7981"));
        assert_eq!(key_path_for(dir, None, 7982), dir.join("control-key.local-7982"));
        if instance_id().is_none() {
            assert_eq!(key_path(dir, 7981), dir.join("control-key.local-7981"));
        }
    }

    #[test]
    fn old_settings_are_copied_once_without_key_files() {
        let root = std::env::temp_dir().join(format!("deskwerk-docs-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let old = root.join(OLD_SETTINGS_NAMES.1);
        let new = root.join("deskwerk-docs");
        std::fs::create_dir_all(old.join("Templates")).unwrap();
        std::fs::write(old.join("ui.json"), "{}").unwrap();
        std::fs::write(old.join("Templates/memo.dotx"), "t").unwrap();
        std::fs::write(old.join("control-key.local-7981"), "k").unwrap();
        assert_eq!(migrate_from(&old, &new).unwrap(), Some(old.clone()));
        assert_eq!(std::fs::read_to_string(new.join("ui.json")).unwrap(), "{}");
        assert_eq!(std::fs::read_to_string(new.join("Templates/memo.dotx")).unwrap(), "t");
        assert!(!new.join("control-key.local-7981").exists());
        assert!(old.join("ui.json").exists(), "the old folder is left alone");
        // The new folder exists now: nothing is copied over it again.
        std::fs::write(old.join("ui.json"), "{\"later\":1}").unwrap();
        assert_eq!(migrate_from(&old, &new).unwrap(), None);
        assert_eq!(std::fs::read_to_string(new.join("ui.json")).unwrap(), "{}");
        // No old folder: nothing to do.
        assert_eq!(migrate_from(&root.join("missing"), &root.join("other")).unwrap(), None);
        assert!(!root.join("other").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn settings_dir_is_the_apps_folder() {
        if let Some(d) = settings_dir() {
            let name = d.file_name().and_then(|n| n.to_str()).unwrap_or("");
            assert!(name == "deskwerk-docs" || name == "Deskwerk Docs", "{}", d.display());
        }
    }
}
