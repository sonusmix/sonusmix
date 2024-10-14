with import <nixpkgs> {};

# minimal shell for cargo-check
mkShell {
  # build deps
  nativeBuildInputs = [
    cargo
    pkg-config
    rustPlatform.bindgenHook
    # cargo-about
  ];

  # library deps
  buildInputs = [
    gtk4
    pipewire
  ];
}
