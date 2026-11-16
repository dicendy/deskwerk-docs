#!/usr/bin/env bash
# Import the Deskwerk Docs icons from the artwork export into assets/app-icon/.
#
# The Deskwerk artwork is drawn outside this repository and has no scalable SVG of the app icon;
# its export directory holds the finished files:
#   app.ico (16-256 px, 16 and 24 hand-drawn), app.icns, app-1024.png, app-macos-512.png,
#   hicolor/<N>x<N>.png (N = 16 24 32 48 64 128 256 512), file.ico and file.svg (document icon).
# The document .icns for macOS is not in the export: render file.svg to 1024 px and pack
# 16-1024 px into assets/app-icon/document.icns (iconutil on macOS, or Pillow).
#
# Needs: python3 with Pillow (for the web manifest icons). The outputs are committed, so builds
# and packaging never need these tools.
#
#   packaging/icons.sh <export-dir>
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="${1:?usage: packaging/icons.sh <export-dir>}"
DIR="$ROOT/assets/app-icon"
ID="eu.deskwerk.Docs"

for f in app.ico app.icns app-1024.png app-macos-512.png file.ico file.svg; do
  [[ -f "$SRC/$f" ]] || { echo "error: $SRC/$f missing" >&2; exit 1; }
done

cp "$SRC/app.ico" "$DIR/wordcraft.ico"           # Windows exe, MSI, shortcuts, web favicon
cp "$SRC/app.icns" "$DIR/wordcraft.icns"         # macOS bundle (Deskwerk Docs.icns)
cp "$SRC/app-1024.png" "$DIR/wordcraft-1024.png"
cp "$SRC/app-macos-512.png" "$DIR/wordcraft-macos-512.png" # macOS window/Dock icon (apps/wordcraft)
cp "$SRC/file.ico" "$DIR/document.ico"           # Windows document ProgIds
cp "$SRC/file.svg" "$DIR/document.svg"           # source of document.icns

# Linux hicolor theme (window icon on Linux/Windows: 256; update prompt: 128).
for s in 16 24 32 48 64 128 256 512; do
  mkdir -p "$DIR/hicolor/${s}x${s}/apps"
  cp "$SRC/hicolor/${s}x${s}.png" "$DIR/hicolor/${s}x${s}/apps/$ID.png"
done

# Web manifest icons (apps/wordcraft-web/manifest.webmanifest).
mkdir -p "$DIR/web"
python3 -I - "$DIR/wordcraft-1024.png" "$DIR/web" <<'EOF'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGBA")
for s in (192, 512):
    im.resize((s, s), Image.Resampling.LANCZOS).save(f"{sys.argv[2]}/icon-{s}.png", optimize=True)
EOF
echo "icons written to $DIR"
