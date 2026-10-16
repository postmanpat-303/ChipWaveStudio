#!/usr/bin/env bash
# Build the Windows x64 zip (mingw cross-compile): exe + README + licences.
# Usage: scripts/package-windows.sh 0.2.2.1
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
