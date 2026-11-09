# ChipWave Studio on Linux

Linux packages are **x86_64 only**. There is no ARM build. App version **0.2.22.0** is on the release [`v0.2.22.0`](https://github.com/postmanpat-303/ChipWaveStudio/releases/tag/v0.2.22.0), together with the Windows files. The 0.2.21.0 Linux files remain on `linux-v0.2.21.0`. The Mac download for 0.2.22.0 is not published: the Mac packaging workflow returned HTTP 403 and was not bypassed. The current Mac app is 0.2.21.0.

The binary is built on **Ubuntu 22.04** and needs **glibc 2.35 or newer**.

| System | Result for 0.2.22.0 |
|---|---|
| Ubuntu 22.04 | `.deb` installed, `--version` and `--self-test` passed, then removed |
| Build host (Ubuntu 24.04) | the same binary's `--version` and `--self-test` passed, and the AppImage `--version` passed via extract-and-run |
| Debian 12, Fedora, openSUSE | not re-tested for 0.2.22.0. The 0.2.21.0 packages were installed on those systems. This build uses the same glibc 2.35 baseline and the same package dependencies |
| ARM, Flathub, Snap | not built |

## Install

**Debian, Ubuntu, Mint** (amd64):

```sh
sudo apt install ./chipwave-studio_0.2.22.0_amd64.deb
```

**Fedora** (and other rpm distros with glibc 2.35 or newer):

```sh
sudo dnf install ./chipwave-studio-0.2.22.0-1.x86_64.rpm
```

**AppImage** (needs the same system libraries as the packages; it does not bundle glibc or the GPU driver):

```sh
chmod +x ChipWave-0.2.22.0-linux-x86_64.AppImage
./ChipWave-0.2.22.0-linux-x86_64.AppImage
```

If FUSE is not installed, the file says it cannot mount. Either install `fuse3` and `libfuse2`, or run:

```sh
./ChipWave-0.2.22.0-linux-x86_64.AppImage --appimage-extract
./squashfs-root/AppRun
```

**Portable archive:** unpack `ChipWave-0.2.22.0-linux-x86_64.tar.gz` and run `./chipwave` from that folder. Same library requirements.

Remove a package with `sudo apt remove chipwave-studio` or `sudo dnf remove chipwave-studio`. Songs and settings under your home directory are left in place.

## Libraries

The program links ALSA (`libasound.so.2`) and opens the display libraries when the window starts: OpenGL (`libGL`, `libEGL`), X11 (`libX11`, `libxcb`, `libXcursor`, `libXi`, `libXrender`, `libxkbcommon`) or Wayland (`libwayland-client`, `libwayland-egl`), and `libdbus`. PipeWire and PulseAudio are used through the distribution’s ALSA plugin; they are not linked directly. CLAP and VST3 plugins are loaded from the folders you choose; they are not bundled.

## Checks

```sh
chipwave --version
chipwave --self-test
chipwave --render demo out.wav
```

The packages are **not signed**. Compare them with `SHA256SUMS-0.2.22.0.txt` on the `v0.2.22.0` release. The 0.2.22.0 `.deb` was installed, `--self-test` passed, and the package was removed on Ubuntu 22.04. The binary needs glibc 2.35 (its highest symbol is `GLIBC_2.35`).
