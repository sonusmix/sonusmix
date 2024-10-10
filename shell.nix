with import <nixpkgs> {};

# shell for dev environment

mkShell {
  # build deps
  nativeBuildInputs = [
    cargo
    pkg-config
    gtk4
    clang
    pipewire
    cargo-about
    cargo-make
    wget # Used for downloading appimagekit, since the version in nixpkgs is old
    resvg
  ];

  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

  buildInputs = [];
}
