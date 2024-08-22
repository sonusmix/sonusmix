with import <nixpkgs> {}; 

# shell for dev environment

mkShell {
  # build deps
  nativeBuildInputs = [
    cargo
    pkg-config
    corepack
    nodejs_18
    openssl
    pango
    gtk3
    pipewire
    clang
    libsoup_3
    webkitgtk_4_1
  ];

  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

  buildInputs = [];
}
