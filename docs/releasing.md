# Releasing Deskwerk Docs

1. Bump the version on `main`: `cargo xtask version set 0.2.0` (the one source of truth is
   `[workspace.package] version`; the app shows it in `--version` and About, with the commit from
   `DESKWERK_DOCS_BUILD_SHA`).
2. Tag the commit `v<version>` and push the tag. `.github/workflows/release.yml` builds the files
   below with the scripts in `packaging/` and drafts a GitHub Release `Deskwerk Docs v<version>`
   with `SHA256SUMS.txt`; review and publish it.

| Platform | Artifacts | Signing |
|---|---|---|
| macOS | universal `Deskwerk Docs.app` in a `.dmg`, universal CLI zip | Developer ID + notarization (`APPLE_*` secrets); ad-hoc without them |
| Windows | x64 `.msi` (one dual-scope package: per-user by default, needs no admin rights; "Everyone on this computer" installs per machine) + portable `.zip` | `WINDOWS_CERTIFICATE` `.pfx` or Azure Trusted Signing (`AZURE_*`); unsigned without them |
| Linux | x86_64 `.AppImage` (+ `.zsync` for AppImageUpdate), `.deb`, `.rpm`, `.tar.gz` | checksums |
| Web | `deskwerk-docs-web-<version>.zip` (static site: `index.html`, wasm, glue) | none |

Missing secrets produce unsigned builds with a warning. Secrets, test runs (`workflow_dispatch`
with a `targets` choice) and what CI does not build: [`packaging/RELEASING.md`](../packaging/RELEASING.md).

Local packaging: `packaging/macos/package.sh --arch universal`, `packaging/windows/package.ps1`,
`packaging/linux/package.sh` (then `packaging/linux/flatpak-bundle.sh` for the Flatpak),
`packaging/web/package.sh`, `packaging/freebsd/package.sh`.
