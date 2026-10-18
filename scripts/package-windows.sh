#!/usr/bin/env bash
# Build the Windows x64 zip (mingw cross-compile): exe + README + licences.
# Also builds the NSIS setup exe when makensis is installed.
# Usage: scripts/package-windows.sh 0.2.2.2
# Release: attach all four files (versioned + versionless ChipWave-setup-x64.exe /
# ChipWave-windows-x64.zip) to the GitHub release; the README links to
# releases/latest/download/<versionless name>.
set -euo pipefail
VER="${1:?version}"
cd "$(dirname "$0")/.."
cargo build --release --target x86_64-pc-windows-gnu
rm -rf dist/pkg && mkdir -p dist/pkg
cp target/x86_64-pc-windows-gnu/release/chipwave.exe dist/pkg/ChipWave.exe
sed "1s/.*/ChipWave Tracker $VER - Windows x64/" dist/README.txt > dist/pkg/README.txt
cp LICENSE dist/pkg/LICENSE.txt
python3 scripts/third_party_licenses.py x86_64-pc-windows-gnu > dist/pkg/THIRD_PARTY_LICENSES.txt
# Windows line endings for Notepad users
for f in dist/pkg/*.txt; do sed -i 's/\r*$/\r/' "$f"; done
rm -f "dist/ChipWave-$VER-windows-x64.zip"
(cd dist/pkg && zip -9 -q "../ChipWave-$VER-windows-x64.zip" ChipWave.exe README.txt LICENSE.txt THIRD_PARTY_LICENSES.txt)
echo "dist/ChipWave-$VER-windows-x64.zip"
# versionless copies: upload these to every release too, so the README's
# releases/latest/download/ChipWave-{setup-x64.exe,windows-x64.zip} links work
cp "dist/ChipWave-$VER-windows-x64.zip" dist/ChipWave-windows-x64.zip
echo "dist/ChipWave-windows-x64.zip"
if command -v makensis >/dev/null; then
  # NSIS VIProductVersion needs four numbers (0.2.3 -> 0.2.3.0)
  VI=$(echo "$VER" | awk -F. '{printf "%d.%d.%d.%d", $1, $2, $3, $4}')
  (cd installer && makensis -V2 -DVERSION="$VER" -DVIVERSION="$VI" -DPKGDIR=../dist/pkg \
    -DOUTFILE="../dist/ChipWave-$VER-setup-x64.exe" chipwave.nsi)
  echo "dist/ChipWave-$VER-setup-x64.exe"
  cp "dist/ChipWave-$VER-setup-x64.exe" dist/ChipWave-setup-x64.exe
  echo "dist/ChipWave-setup-x64.exe"
fi
