with import <nixpkgs> {};

# shell for dev environment

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
  ];

  # LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

  buildInputs = [
  # build deps
    gtk4
    pipewire
  ];
}
