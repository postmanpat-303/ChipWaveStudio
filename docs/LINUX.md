# ChipWave Studio on Linux

Linux packages are **x86_64 only**. There is no ARM build. They are app version **0.2.21.0**, published on the release `linux-v0.2.21.0`. That release is not “latest”: the Windows and Mac downloads stay on their own release until a Mac build of this version exists.

The binary is built on **Ubuntu 22.04** and needs **glibc 2.35 or newer**.

| System | Result |
|---|---|
| Ubuntu 22.04 and 24.04 | `.deb` installed, `--version` and `--self-test` passed, then removed |
| Debian 12 | same |
| Fedora 41 | `.rpm` installed, `--version` and `--self-test` passed, then removed |
| openSUSE Leap 15.6 | glibc 2.38. The `.rpm` installed with `zypper`, `--version` ran, and the same binary’s `--self-test` passed |
| ARM, Flathub, Snap | not built |

## Install

**Debian, Ubuntu, Mint** (amd64):

```sh
sudo apt install ./chipwave-studio_0.2.21.0_amd64.deb
```

**Fedora** (and other rpm distros with glibc 2.35 or newer):

```sh
sudo dnf install ./chipwave-studio-0.2.21.0-1.x86_64.rpm
```

**AppImage** (needs the same system libraries as the packages; it does not bundle glibc or the GPU driver):

```sh
chmod +x ChipWave-0.2.21.0-linux-x86_64.AppImage
./ChipWave-0.2.21.0-linux-x86_64.AppImage
```

If FUSE is not installed, the file says it cannot mount. Either install `fuse3` and `libfuse2`, or run:

```sh
./ChipWave-0.2.21.0-linux-x86_64.AppImage --appimage-extract
./squashfs-root/AppRun
```

**Portable archive:** unpack `ChipWave-0.2.21.0-linux-x86_64.tar.gz` and run `./chipwave` from that folder. Same library requirements.

Remove a package with `sudo apt remove chipwave-studio` or `sudo dnf remove chipwave-studio`. Songs and settings under your home directory are left in place.

## Libraries

The program links ALSA (`libasound.so.2`) and opens the display libraries when the window starts: OpenGL (`libGL`, `libEGL`), X11 (`libX11`, `libxcb`, `libXcursor`, `libXi`, `libXrender`, `libxkbcommon`) or Wayland (`libwayland-client`, `libwayland-egl`), and `libdbus`. PipeWire and PulseAudio are used through the distribution’s ALSA plugin; they are not linked directly. CLAP and VST3 plugins are loaded from the folders you choose; they are not bundled.

## Checks

```sh
chipwave --version
chipwave --self-test
chipwave --render demo out.wav
```

The packages are **not signed**. Compare them with `SHA256SUMS-linux-0.2.21.0.txt` from the same release.
