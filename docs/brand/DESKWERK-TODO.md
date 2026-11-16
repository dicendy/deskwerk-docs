# Deskwerk Docs: rebranding leftovers

Deskwerk Docs is based on WordCraft by the ArtCraft team (MIT OR Apache-2.0). The user-facing
name, app id (`eu.deskwerk.Docs`), binaries (`deskwerk-docs`, `deskwerk-docs-cli`), packages,
installers and settings folders are renamed. This file lists what still carries the upstream look
or name, and why.

## Art

The app and document icons are the Deskwerk artwork (since 0.15.5), imported from the artwork
export by `packaging/icons.sh <export-dir>`:

- `assets/app-icon/wordcraft.ico` (Windows exe, MSI, shortcuts and Apps list; web favicon)
- `assets/app-icon/wordcraft.icns` (copied into the bundle as `Deskwerk Docs.icns`)
- `assets/app-icon/wordcraft-1024.png`, `assets/app-icon/web/icon-192.png`, `icon-512.png` (web
  manifest)
- `assets/app-icon/wordcraft-macos-512.png` (macOS window/Dock icon, embedded by `apps/wordcraft`)
- `assets/app-icon/hicolor/*/apps/eu.deskwerk.Docs.png` (Linux; 256 is also the Windows/Linux
  window icon, 128 the update prompt's). There is no scalable SVG of the app icon.
- `assets/app-icon/document.ico` / `document.icns` / `document.svg`: the document icon of the Word,
  RTF and ODT types (Windows ProgIds, macOS `CFBundleTypeIconFile`).

Still upstream's: the app colours, accent `#3f5fe0` (`UPDATE_APP` in `crates/ui-egui/src/lib.rs`)
and the metainfo `<branding>` colours `#b9c7f5` / `#2b47b5`. Linux ships no MIME-type icons: the
types are the system's (`application/vnd.openxmlformats-…`), and an icon for them would change how
every .docx looks, whichever app opens it.

## macOS DMG layout

- `packaging/macos/dmg/dmg-layout.DS_Store` still describes a volume named `WordCraft` with
  `WordCraft.app`. The volume and bundle are now `Deskwerk Docs` / `Deskwerk Docs.app`, so
  Finder falls back to default icon positions until it is regenerated with
  `packaging/macos/dmg/generate.py` (already updated) on a Mac.
- `packaging/macos/dmg/background.svg` / `background.tiff` still show the old WordCraft icon
  (`packaging/macos/dmg/wordcraft-icon.svg`) and name; redraw them for the Deskwerk icon.

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
  icon file names above (`wordcraft.ico`, …).
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

- `.github/workflows/release.yml` (tag `v*` or manual run) builds Linux, web, Windows and macOS
  packages and drafts a GitHub Release. No signing identity is configured yet, so Windows files
  are unsigned and macOS is ad-hoc signed, not notarized. To sign, add these repository secrets
  (details in `packaging/RELEASING.md`):
  - Windows, either `WINDOWS_CERTIFICATE` (base64 `.pfx` code-signing certificate) +
    `WINDOWS_CERTIFICATE_PASSWORD`, or Azure Trusted Signing: `AZURE_TENANT_ID`,
    `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_SIGNING_ENDPOINT`, `AZURE_SIGNING_ACCOUNT`,
    `AZURE_CERT_PROFILE`.
  - macOS signing: `APPLE_CERTIFICATE` (base64 `.p12` "Developer ID Application" certificate with
    key), `APPLE_CERTIFICATE_PASSWORD`, optional `KEYCHAIN_PASSWORD`.
  - macOS notarization: `APPLE_ID`, `APPLE_PASSWORD` (app-specific password), `APPLE_TEAM_ID`.
- Not built by CI yet: Linux aarch64/riscv64, Flatpak, FreeBSD, Windows x86/arm64. No Linux
  package signing (upstream's `LINUX_SIGNING_KEY`); releases carry `SHA256SUMS.txt` only.

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
