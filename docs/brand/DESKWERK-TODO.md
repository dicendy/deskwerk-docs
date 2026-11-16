# Deskwerk Docs: rebranding leftovers

Deskwerk Docs is a modified version of an open-source word processor (MIT OR Apache-2.0); the
upstream project and its authors are named in NOTICE and ATTRIBUTION.md. Since 0.15.6 the upstream
name is gone from the code, crates, binaries, packages, installers, settings folders, environment
variables, workflows and documentation. This file lists what still carries the upstream look or
name, and why.

## Art

The app and document icons are the Deskwerk artwork (since 0.15.5), imported from the artwork
export by `packaging/icons.sh <export-dir>`:

- `assets/app-icon/deskwerk-docs.ico` (Windows exe, MSI, shortcuts and Apps list; web favicon)
- `assets/app-icon/deskwerk-docs.icns` (copied into the bundle as `Deskwerk Docs.icns`)
- `assets/app-icon/deskwerk-docs-1024.png`, `assets/app-icon/web/icon-192.png`, `icon-512.png` (web
  manifest; the 1024 also draws the DMG background)
- `assets/app-icon/deskwerk-docs-macos-512.png` (macOS window/Dock icon, embedded by `apps/deskwerk-docs`)
- `assets/app-icon/hicolor/*/apps/eu.deskwerk.Docs.png` (Linux; 256 is also the Windows/Linux
  window icon, 128 the update prompt's). There is no scalable SVG of the app icon.
- `assets/app-icon/document.ico` / `document.icns` / `document.svg`: the document icon of the Word,
  RTF and ODT types (Windows ProgIds, macOS `CFBundleTypeIconFile`).

Still upstream's: the app colours, accent `#3f5fe0` (`UPDATE_APP` in `crates/ui-egui/src/lib.rs`)
and the metainfo `<branding>` colours `#b9c7f5` / `#2b47b5`. Linux ships no MIME-type icons: the
types are the system's (`application/vnd.openxmlformats-…`), and an icon for them would change how
every .docx looks, whichever app opens it.

## Upstream names kept on purpose

- LICENSE*, NOTICE, ATTRIBUTION.md and the third-party licence listings name the upstream project
  and its authors, as the licences require.
- Compatibility shims, each a commented constant, read the upstream name and write ours:
  - settings: on first start the desktop app copies upstream's settings folder when ours does not
    exist yet (`crates/control-key/src/lib.rs`, `migrate_old_settings`);
  - native `.json` documents and recovery snapshots: upstream's passthrough key prefix, ODF object
    marker and canvas-rotation extension URI are still read (`crates/doc/src/legacy.rs`);
  - web: upstream's local-storage key for the interface preferences is still read
    (`apps/deskwerk-docs-web/src/prefs.rs`).
- `.github/i18n/deskwerk.patch` renames upstream's catalog files and product names in craft-i18n
  at build time, so its rename and removed lines carry the old names.
- craft-proof (storytold/craft-libs) still reads its own dictionaries environment variable and
  per-app dictionaries folder under upstream's name; Deskwerk Docs also reads
  `DESKWERK_DOCS_DICTIONARIES` and File › Options › Language's folder. Changing craft-proof's
  names needs a craft-libs change.
- The web build's font store (IndexedDB `deskwerk-docs`) is new in 0.15.6, so fonts downloaded
  in the web app before are fetched again.

## Windows installer identity

- New UpgradeCode `78337589-0956-416B-B1C6-FA48562A3C8C` (fixed from now on; it does not upgrade
  upstream installs) and new component GUIDs. Manufacturer `Deskwerk`, folder
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
- craft-update (storytold/craft-libs) still assumes upstream's conventions when it looks for an
  installed copy to replace: upstream's MSI publisher and the `ai.storyteller.<id>` macOS
  bundle id. So in-place install from the update prompt may not find Deskwerk installs and fall
  back to a download. Fixing it needs a craft-update change (or disabling the menu item).
