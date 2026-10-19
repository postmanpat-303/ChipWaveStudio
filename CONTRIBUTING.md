# Contributing to ChipWave Studio

Thanks for wanting to help. ChipWave Studio is free software under the GPL-3.0-or-later, made by Del (postmanpat) and contributors. Fixes, features, docs and sounds are all welcome.

## Finding something to do

- [docs/ROADMAP.md](docs/ROADMAP.md) lists the things to add in the future, each with a size and a file to start from. The **Good first picks** at the top are a friendly way in.
- **Before you start, open an issue (or comment on one) to claim the item,** so nobody else builds the same thing. For **[big]** items, sketch your plan in the issue first.
- Bug reports are welcome too: say what you did, what happened, your OS and the ChipWave version (Help › About).

## Building

You need a recent stable Rust (`rustup`).

```
# Linux needs these packages (Debian/Ubuntu names; the same list CI installs):
sudo apt-get install libasound2-dev libxkbcommon-dev libwayland-dev libx11-dev \
  libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev

cargo run --release                                    # build and run
cargo build --release --target x86_64-pc-windows-gnu   # Windows exe from Linux (mingw-w64)
cargo run --release -- --self-test                     # headless check: renders the demo
```

Packaging for Windows, Linux and macOS is in [docs/PACKAGING.md](docs/PACKAGING.md) and [docs/MACOS.md](docs/MACOS.md).

## Before you open a pull request

CI runs these three on Linux, Windows and macOS, so run them locally first:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- **Old songs must play bit-identically.** A change to the engine, an effect or an instrument must not change the sound of existing `.cwt` songs unless the new behaviour is switched on. The render tests check this, so add one if your change touches sound.
- **Avoid song format changes.** If you really need one, bump the version carefully and keep loading older files (see [docs/FILE_FORMAT.md](docs/FILE_FORMAT.md)).
- **Add tests** for new behaviour; the existing `*_tests.rs` files show the style, including UI tests with `egui_kittest`.
- **Update the docs:** a line in [docs/CHANGELOG.md](docs/CHANGELOG.md), the guide in `docs/` for the area you touched, and remove your item from [docs/ROADMAP.md](docs/ROADMAP.md) (and from that guide's Limits list).
- Keep each pull request to one thing, and say in it which issue it closes.

## Credit open-source work

If your change ports, follows or borrows from someone else's code, design or samples, check that the licence is compatible with GPL-3.0-or-later and credit them: in the README's credits list, in the source file where the code lives, and in the dependency licence list for new crates (`cargo deny` checks licences in CI). When in doubt, ask in the issue.

## Licence

By contributing you agree that your work is released under the GPL-3.0-or-later, the same as the rest of ChipWave Studio.
