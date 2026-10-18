#!/usr/bin/env bash
# Build ChipWave.app (universal: Apple Silicon + Intel), ad-hoc signed, as a .dmg and a .zip.
# Runs on macOS only (CI: .github/workflows/macos.yml, started by hand).
#   dist/mac/ChipWave-<ver>-macos-universal.dmg   (app + Applications link + read-me + licences)
#   dist/mac/ChipWave-<ver>-macos-universal.zip   (the .app)
#   dist/mac/ChipWave-macos-universal.{dmg,zip}   (versionless copies for releases/latest/download/)
#   dist/mac/SHA256SUMS.txt
# <ver> is the display version (Cargo 0.2.3+fix.15 -> 0.2.3.15); default: from Cargo.toml.
# Release: attach all four files (and merge SHA256SUMS.txt) to the GitHub release; the README
# links to releases/latest/download/ChipWave-macos-universal.dmg.
# Usage: scripts/package-macos.sh [0.2.3.15]
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_VER=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
DISPLAY_VER=$(echo "$CARGO_VER" | sed 's/+fix\./\./')
VER="${1:-$DISPLAY_VER}"
[ "$VER" = "$DISPLAY_VER" ] || { echo "version $VER does not match Cargo.toml ($CARGO_VER = $DISPLAY_VER)" >&2; exit 1; }
SHORT=$(echo "$VER" | cut -d. -f1-3)
export MACOSX_DEPLOYMENT_TARGET=11.0
for t in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build --release --locked --target "$t"
done

OUT="dist/mac"; rm -rf "$OUT"; mkdir -p "$OUT"
APP="$OUT/ChipWave.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/ChipWave" \
  target/aarch64-apple-darwin/release/chipwave target/x86_64-apple-darwin/release/chipwave
lipo -info "$APP/Contents/MacOS/ChipWave"
sed -e "s/@VERSION@/$VER/g" -e "s/@SHORTVERSION@/$SHORT/g" packaging/macos/Info.plist.in > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
printf 'APPL????' > "$APP/Contents/PkgInfo"

# Icon: an .iconset from the 1024 px app icon, then iconutil.
SET="$(mktemp -d)/ChipWave.iconset"; mkdir -p "$SET"
SRC=assets/icon/chipwave_studio_1024.png
for s in 16 32 128 256 512; do
  sips -z $s $s "$SRC" --out "$SET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2)); sips -z $d $d "$SRC" --out "$SET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$SET" -o "$APP/Contents/Resources/ChipWave.icns"

# Licences + read-me inside the bundle too (demos, presets and the icon are compiled into the binary).
DOCS="$(mktemp -d)"
sed "s/@VERSION@/$VER/g" packaging/macos/README.txt > "$DOCS/README.txt"
cp LICENSE "$DOCS/LICENSE.txt"
python3 scripts/third_party_licenses.py aarch64-apple-darwin > "$DOCS/THIRD_PARTY_LICENSES.txt"
cp "$DOCS"/*.txt "$APP/Contents/Resources/"

# Not notarised, but ad-hoc signed so Apple Silicon will run it.
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

NAME="ChipWave-$VER-macos-universal"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$OUT/$NAME.zip"
DMGDIR="$(mktemp -d)"
cp -R "$APP" "$DMGDIR/"
ln -s /Applications "$DMGDIR/Applications"
cp "$DOCS/README.txt" "$DMGDIR/Read me first.txt"
cp "$DOCS/LICENSE.txt" "$DOCS/THIRD_PARTY_LICENSES.txt" "$DMGDIR/"
# hdiutil sometimes fails with "Resource busy" on CI runners; try a few times.
ok=0
for i in 1 2 3 4 5 6; do
  if hdiutil create -volname "ChipWave Studio $VER" -srcfolder "$DMGDIR" -fs HFS+ -format UDZO -ov "$OUT/$NAME.dmg"; then
    ok=1; break
  fi
  echo "hdiutil failed (try $i), retrying"; sleep $((i * 5))
done
[ "$ok" = 1 ] && test -f "$OUT/$NAME.dmg"
rm -rf "$DMGDIR"

cp "$OUT/$NAME.dmg" "$OUT/ChipWave-macos-universal.dmg"
cp "$OUT/$NAME.zip" "$OUT/ChipWave-macos-universal.zip"
(cd "$OUT" && shasum -a 256 "$NAME.dmg" "$NAME.zip" ChipWave-macos-universal.dmg ChipWave-macos-universal.zip > SHA256SUMS.txt && cat SHA256SUMS.txt)
ls -la "$OUT"
