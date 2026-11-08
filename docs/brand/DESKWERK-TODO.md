# Deskwerk Docs: rebranding leftovers

Deskwerk Docs is based on WordCraft by the ArtCraft team (MIT OR Apache-2.0). The user-facing
name, app id (`eu.deskwerk.Docs`), binaries (`deskwerk-docs`, `deskwerk-docs-cli`), packages,
installers and settings folders are renamed. This file lists what still carries the upstream look
or name, and why.

## Art to replace

These icons are the original placeholder art from the upstream repository (a document page and pen
on a blue tile, MIT OR Apache-2.0, see `ATTRIBUTION.md`). They are **not** ArtCraft trademarks, so
shipping them is allowed, but Deskwerk should get its own icon:

- `assets/app-icon/wordcraft.svg` (the source; everything below is generated from it by
  `packaging/icons.sh`)
- `assets/app-icon/wordcraft-1024.png`
- `assets/app-icon/wordcraft-macos-512.png` (macOS Dock icon, embedded by `apps/wordcraft`)
- `assets/app-icon/wordcraft.icns` (copied into the bundle as `Deskwerk Docs.icns`)
- `assets/app-icon/wordcraft.ico` (Windows exe and MSI icon, `cargo xtask ico`)
- `assets/app-icon/hicolor/*/apps/eu.deskwerk.Docs.png` and
  `assets/app-icon/hicolor/scalable/apps/eu.deskwerk.Docs.svg`
- The app colours: accent `#3f5fe0` (`UPDATE_APP` in `crates/ui-egui/src/lib.rs`) and the
  metainfo `<branding>` colours `#b9c7f5` / `#2b47b5`.

After replacing the SVG: run `packaging/icons.sh` and update the `ATTRIBUTION.md` rows
(`cargo xtask assets` fails on any asset without a row).

## macOS DMG layout

- `packaging/macos/dmg/dmg-layout.DS_Store` still describes a volume named `WordCraft` with
  `WordCraft.app`. The volume and bundle are now `Deskwerk Docs` / `Deskwerk Docs.app`, so
  Finder falls back to default icon positions until it is regenerated with
  `packaging/macos/dmg/generate.py` (already updated) on a Mac.
- `packaging/macos/dmg/background.svg` / `background.tiff` embed the app icon; regenerate them
  with the new icon.

## Internal names kept on purpose

- Cargo packages, crate names, module paths and directories: `wordcraft`, `wordcraft-cli`,
  `wordcraft-web`, `wordcraft-*` crates, `apps/wordcraft*` (`cargo -p wordcraft…` keeps working,
  `Cargo.lock` is unchanged).
- The web build (`wordcraft-web` binary and its hashed `wordcraft-web-<hash>.js` / `.wasm`), its
  page messages (`wordcraft:ready`, …) and `window.wordcraft`, which are a host-page API. Only the
  zip is renamed (`deskwerk-docs-web-<version>.zip`).
- `WORDCRAFT_*` environment variables.
- Log targets (`wordcraft_*`; the app binary now logs as `deskwerk_docs`, which the default
  filter includes) and the log file name `wordcraft.log`.
- `wordcraft://` resource URIs of the MCP server and `wordcraft:*` keys kept in saved documents.
- File names `packaging/windows/wordcraft.wxs` and internal WiX ids (`WordcraftExe`, …), and the
  icon source file names above.
- The user spelling-dictionary folder (`…\WordCraft\dictionaries`, see
  `docs/spelling-dictionaries.md`): the name comes from craft-proof (storytold/craft-libs), not
  from this repository. Not verified here.
- The craft-i18n catalogs are upstream's; `.github/i18n/deskwerk.patch` renames the product and
  CLI names in them at build time.

## Windows installer identity

- New UpgradeCode `78337589-0956-416B-B1C6-FA48562A3C8C` (fixed from now on; it does not upgrade
  upstream WordCraft installs) and new component GUIDs. Manufacturer `Deskwerk`, folder
  `Deskwerk\Docs`, ProgIds `DeskwerkDocs.*`, registry `Software\DeskwerkDocs`.

## Release signing

- This fork has no `release.yml`. Deskwerk needs its own Apple Developer ID and Windows signing
  identity; none is configured.

## Updates

- No startup update check. Help › Check for Updates… uses craft-update with `UPDATE_APP`
  (`crates/ui-egui/src/lib.rs`): id `deskwerk-docs`, repo `dicendy/deskwerk-docs`, so it only
  asks `https://api.github.com/repos/dicendy/deskwerk-docs/releases`. The AppImage update
  information points at the same releases.
- craft-update (storytold/craft-libs) still assumes ArtCraft conventions when it looks for an
  installed copy to replace: the ArtCraft MSI publisher and the `ai.storyteller.<id>` macOS
  bundle id. So in-place install from the update prompt may not find Deskwerk installs and fall
  back to a download. Fixing it needs a craft-update change (or disabling the menu item).
- The AUR and Gentoo packages listed in the README package upstream WordCraft, not this fork.
