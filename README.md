```mermaid
graph TD
    A(["⚠️ Sonusmix is in development. It should be fairly stable by now, but it is still missing some crucial features."])
```

# Sonusmix
A tool to easily route devices in Pipewire. It intends to enable the same features and workflows as [Voicemeeter](https://vb-audio.com/Voicemeeter/) or [Pulsemeeter](https://github.com/theRealCarneiro/pulsemeeter), but with a more intuitive interface.

## Installation
We are working on shipping prebuilt AppImages, and distributing to flathub. Until then, you can build the project with Nix, or install the dependencies manually:
- Rust and `cargo`
- `pipewire`
- `gtk4`
- `cargo-make` (from your system package manager or `cargo install --locked cargo-make`)
- `cargo-about` (from your system package manager or `cargo install --locked cargo-about`)
- `resvg` (from your system package manager or `cargo install --locked resvg`)
- `pax-utils` (only if building an AppImage)
- `appimagetool` (only if building an AppImage, this will be automatically downloaded if needed as long as `wget` is installed)
- `flatpak` and `flatpak-builder` (only if building a Flatpak)

To build an AppImage after installing the dependencies, run:
```bash
git clone https://codeberg.org/sonusmix/sonusmix
cd sonusmix
cargo make build-appimage
```

Or, to build an AppImage using nix (without needing to install any other dependencies), run:
```bash
git clone https://codeberg.org/sonusmix/sonusmix
cd sonusmix
nix-shell shell.nix --pure --command "cargo make build-appimage"
```

Building a Flatpak is possible, but does not work under nix. You will have to install the dependencies above, and then run:
```bash
git clone https://codeberg.org/sonusmix/sonusmix
cd sonusmix
cargo make install-flatpak # or cargo make build-flatpak to build without installing
```
