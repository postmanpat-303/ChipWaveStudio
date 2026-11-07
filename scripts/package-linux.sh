#!/usr/bin/env bash
# Linux x86_64 packages for ChipWave Studio.
# The binary must be built on Ubuntu 22.04 (glibc 2.35), not a newer host.
# See docs/LINUX.md.
#
# Usage:
#   scripts/package-linux.sh [version]
# Env:
#   CW_LINUX_BIN   release binary (default: target/ubuntu2204/release/chipwave)
set -euo pipefail
cd "$(dirname "$0")/.."
VER="${1:-}"
if [ -z "$VER" ]; then
  CARGO_VER=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
  VER=$(echo "$CARGO_VER" | sed 's/+fix\./\./')
fi
BIN="${CW_LINUX_BIN:-target/ubuntu2204/release/chipwave}"
[ -x "$BIN" ] || { echo "missing binary $BIN" >&2; exit 1; }
file "$BIN" | grep -q 'x86-64' || { echo "$BIN is not x86-64" >&2; exit 1; }

MAX_GLIBC=$(objdump -T "$BIN" | awk '/GLIBC_/ {gsub(/[()]/,"",$5); print $5}' | sort -Vu | tail -1)
echo "glibc symbols up to $MAX_GLIBC"

STAGE=dist/linux
rm -rf "$STAGE"
APP="$STAGE/AppDir"
ID=io.github.postmanpat_303.ChipWaveStudio
mkdir -p "$APP/usr/bin" \
  "$APP/usr/share/applications" \
  "$APP/usr/share/metainfo" \
  "$APP/usr/share/mime/packages" \
  "$APP/usr/share/doc/chipwave-studio"
cp "$BIN" "$APP/usr/bin/chipwave"
chmod 755 "$APP/usr/bin/chipwave"
cp "packaging/linux/$ID.desktop" "$APP/usr/share/applications/$ID.desktop"
cp "packaging/linux/$ID.metainfo.xml" "$APP/usr/share/metainfo/$ID.metainfo.xml"
cp packaging/linux/chipwave.xml "$APP/usr/share/mime/packages/chipwave.xml"
for s in 16 32 48 64 128 256 512; do
  mkdir -p "$APP/usr/share/icons/hicolor/${s}x${s}/apps"
  cp "assets/icon/chipwave_studio_${s}.png" \
    "$APP/usr/share/icons/hicolor/${s}x${s}/apps/$ID.png"
done
cp LICENSE "$APP/usr/share/doc/chipwave-studio/copyright"
python3 scripts/third_party_licenses.py x86_64-unknown-linux-gnu \
  > "$APP/usr/share/doc/chipwave-studio/THIRD_PARTY_LICENSES.txt"
cat > "$APP/usr/share/doc/chipwave-studio/README.txt" <<EOF
ChipWave Studio $VER for Linux x86_64
=====================================
Free software, GPL-3.0-or-later.
https://github.com/postmanpat-303/ChipWaveStudio

This build needs glibc $MAX_GLIBC or newer (Ubuntu 22.04, Debian 12,
Fedora 36 and newer). It is not an ARM build.

It uses the system's sound and display libraries: ALSA (PipeWire or
PulseAudio through the ALSA plugin is fine), OpenGL, and X11 or Wayland.
Install those with the .deb or .rpm. The portable archive and the
AppImage need the same libraries; they do not bundle glibc or the GPU
driver.

Run: chipwave
Version: chipwave --version
Self-test: chipwave --self-test
Export the demo: chipwave --render demo out.wav

Songs and settings stay in ~/.config/chipwave and are not removed on
uninstall. The packages are not signed.
EOF
# Unix line endings in the package docs. The Windows script converts its own.

# ---- portable tar.gz ----
PORT="$STAGE/ChipWave-$VER-linux-x86_64"
rm -rf "$PORT"
mkdir -p "$PORT"
cp "$APP/usr/bin/chipwave" "$PORT/chipwave"
cp "$APP/usr/share/doc/chipwave-studio/README.txt" "$PORT/README.txt"
cp "$APP/usr/share/doc/chipwave-studio/copyright" "$PORT/LICENSE.txt"
cp "$APP/usr/share/doc/chipwave-studio/THIRD_PARTY_LICENSES.txt" "$PORT/THIRD_PARTY_LICENSES.txt"
tar -C "$STAGE" -czf "dist/ChipWave-$VER-linux-x86_64.tar.gz" "ChipWave-$VER-linux-x86_64"
echo "dist/ChipWave-$VER-linux-x86_64.tar.gz"

# ---- .deb ----
DEB="$STAGE/deb"
rm -rf "$DEB"
mkdir -p "$DEB/DEBIAN"
cp -a "$APP/usr" "$DEB/usr"
cat > "$DEB/DEBIAN/control" <<EOF
Package: chipwave-studio
Version: $VER
Section: sound
Priority: optional
Architecture: amd64
Maintainer: Del (postmanpat) <postmanpat-303@users.noreply.github.com>
Depends: libasound2t64 (>= 1.2.2) | libasound2 (>= 1.2.2), libegl1, libgl1, libx11-6, libx11-xcb1, libxcb1, libxcursor1, libxi6, libxrender1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-egl1, libdbus-1-3, libgcc-s1
Homepage: https://github.com/postmanpat-303/ChipWaveStudio
Description: Free music studio with built-in synths
 ChipWave Studio $VER for Linux x86_64. Needs glibc $MAX_GLIBC or newer.
 Songs are saved as .cwt. This package is not signed.
EOF
cat > "$DEB/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications || true
command -v update-mime-database >/dev/null 2>&1 && update-mime-database /usr/share/mime || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q /usr/share/icons/hicolor || true
EOF
cp "$DEB/DEBIAN/postinst" "$DEB/DEBIAN/postrm"
chmod 755 "$DEB/DEBIAN/postinst" "$DEB/DEBIAN/postrm"
DEB_OUT="dist/chipwave-studio_${VER}_amd64.deb"
dpkg-deb --root-owner-group -Zxz --build "$DEB" "$DEB_OUT"
echo "$DEB_OUT"

# ---- .rpm ----
RPMBUILD="$STAGE/rpmbuild"
rm -rf "$RPMBUILD"
mkdir -p "$RPMBUILD/BUILD" "$RPMBUILD/RPMS" "$RPMBUILD/SOURCES" "$RPMBUILD/SPECS" "$RPMBUILD/BUILDROOT"
# ship the staged /usr tree as a tarball the spec unpacks
tar -C "$APP" -czf "$RPMBUILD/SOURCES/chipwave-studio.tar.gz" usr
cat > "$RPMBUILD/SPECS/chipwave-studio.spec" <<EOF
Name: chipwave-studio
Version: $VER
Release: 1
Summary: Free music studio with built-in synths
License: GPL-3.0-or-later
URL: https://github.com/postmanpat-303/ChipWaveStudio
Source0: chipwave-studio.tar.gz
BuildArch: x86_64
# Sonames, so Fedora and openSUSE can each pick their own package.
Requires: libasound.so.2()(64bit)
Requires: libGL.so.1()(64bit)
Requires: libEGL.so.1()(64bit)
Requires: libX11.so.6()(64bit)
Requires: libX11-xcb.so.1()(64bit)
Requires: libxcb.so.1()(64bit)
Requires: libXcursor.so.1()(64bit)
Requires: libXi.so.6()(64bit)
Requires: libXrender.so.1()(64bit)
Requires: libxkbcommon.so.0()(64bit)
Requires: libxkbcommon-x11.so.0()(64bit)
Requires: libwayland-client.so.0()(64bit)
Requires: libwayland-egl.so.1()(64bit)
Requires: libdbus-1.so.3()(64bit)
Requires: libgcc_s.so.1()(64bit)

%description
ChipWave Studio $VER for Linux x86_64. Needs glibc $MAX_GLIBC or newer.
Songs and settings under the user account are left in place on uninstall.
This package is not signed.

%prep
%setup -q -c

%install
mkdir -p %{buildroot}
cp -a usr %{buildroot}/

%files
/usr/bin/chipwave
/usr/share/applications/$ID.desktop
/usr/share/metainfo/$ID.metainfo.xml
/usr/share/mime/packages/chipwave.xml
/usr/share/icons/hicolor/*/apps/$ID.png
/usr/share/doc/chipwave-studio/copyright
/usr/share/doc/chipwave-studio/README.txt
/usr/share/doc/chipwave-studio/THIRD_PARTY_LICENSES.txt

%post
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications || true
command -v update-mime-database >/dev/null 2>&1 && update-mime-database /usr/share/mime || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q /usr/share/icons/hicolor || true

%postun
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications || true
command -v update-mime-database >/dev/null 2>&1 && update-mime-database /usr/share/mime || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q /usr/share/icons/hicolor || true
EOF
rpmbuild --define "_topdir $PWD/$RPMBUILD" -bb "$RPMBUILD/SPECS/chipwave-studio.spec"
RPM_BUILT=$(find "$RPMBUILD/RPMS" -name '*.rpm' -print)
cp "$RPM_BUILT" "dist/chipwave-studio-$VER-1.x86_64.rpm"
echo "dist/chipwave-studio-$VER-1.x86_64.rpm"

# ---- AppImage (system GUI/ALSA libs, no bundled glibc) ----
APPRUN="$APP/AppRun"
cat > "$APPRUN" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export PATH="$HERE/usr/bin:$PATH"
exec "$HERE/usr/bin/chipwave" "$@"
EOF
chmod 755 "$APPRUN"
cp "$APP/usr/share/applications/$ID.desktop" "$APP/$ID.desktop"
cp "$APP/usr/share/icons/hicolor/256x256/apps/$ID.png" "$APP/$ID.png"
# A second name some desktops look up from the desktop Icon= key.
cp "$APP/$ID.png" "$APP/.DirIcon"
TOOLDIR="$STAGE/tools"
mkdir -p "$TOOLDIR"
TOOL="$TOOLDIR/appimagetool-x86_64.AppImage"
if [ ! -x "$TOOL" ]; then
  curl -fsSL -o "$TOOL" \
    https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$TOOL"
fi
export APPIMAGE_EXTRACT_AND_RUN=1
ARCH=x86_64 "$TOOL" "$APP" "dist/ChipWave-$VER-linux-x86_64.AppImage"
echo "dist/ChipWave-$VER-linux-x86_64.AppImage"

# checksums of the four Linux files only
(
  cd dist
  sha256sum \
    "ChipWave-$VER-linux-x86_64.tar.gz" \
    "ChipWave-$VER-linux-x86_64.AppImage" \
    "chipwave-studio_${VER}_amd64.deb" \
    "chipwave-studio-$VER-1.x86_64.rpm" \
    > "SHA256SUMS-linux-$VER.txt"
  cat "SHA256SUMS-linux-$VER.txt"
)
echo "glibc $MAX_GLIBC"
