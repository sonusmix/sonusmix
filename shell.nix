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
  ];

  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

  buildInputs = [];
}
