#!/usr/bin/env bash
# Write <dir>/SHA256SUMS.txt for the release files of version <ver> in <dir>:
#   ChipWave-<ver>-setup-x64.exe, ChipWave-<ver>-windows-x64.zip, ChipWave-<ver>-macos-universal.{dmg,zip}
#   and the versionless copies ChipWave-setup-x64.exe, ChipWave-windows-x64.zip,
#   ChipWave-macos-universal.{dmg,zip} (whichever are present).
# Standard "sha256  filename" lines (sha256sum -c / shasum -a 256 -c can check them).
# Wave Hub verifies downloads against this file, so attach it to every release.
# Usage: scripts/release-sums.sh dist 0.2.3.16
set -euo pipefail
DIR="${1:?dir}"; VER="${2:?version}"
cd "$DIR"
files=()
for f in "ChipWave-$VER-setup-x64.exe" "ChipWave-$VER-windows-x64.zip" \
         ChipWave-setup-x64.exe ChipWave-windows-x64.zip \
         "ChipWave-$VER-macos-universal.dmg" "ChipWave-$VER-macos-universal.zip" \
         ChipWave-macos-universal.dmg ChipWave-macos-universal.zip; do
  [ -f "$f" ] && files+=("$f")
done
[ ${#files[@]} -gt 0 ] || { echo "no release files for $VER in $DIR" >&2; exit 1; }
if command -v sha256sum >/dev/null; then sha256sum "${files[@]}" > SHA256SUMS.txt
else shasum -a 256 "${files[@]}" > SHA256SUMS.txt; fi
cat SHA256SUMS.txt
echo "$DIR/SHA256SUMS.txt (${#files[@]} files)"
