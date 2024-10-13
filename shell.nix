with import <nixpkgs> {};

# shell for dev environment

let
  appimagetool = import ./assets/appimagetool.nix {};
in
mkShell {
  # build deps
  nativeBuildInputs = [
    cargo
    pkg-config
    rustPlatform.bindgenHook
    cargo-about
    cargo-make
    wget # Used for downloading appimagekit, since the version in nixpkgs is old
    resvg
    pax-utils
    appimagetool
  ];

  buildInputs = [
    # library deps
    gtk4
    pipewire
  ];
}
