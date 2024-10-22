{ pkgs ? import <nixpkgs> { } }:
pkgs.rustPlatform.buildRustPackage {
  pname = "sonusmix";
  version = "0.1.0";
  doCheck = false;
  cargoLock = {
    lockFile = ./Cargo.lock;
    outputHashes = {
      "libspa-0.8.0" = "sha256-R68TkFbzDFA/8Btcar+0omUErLyBMm4fsmQlCvfqR9o=";
    };
  };
  src = pkgs.lib.cleanSource ./.;
  nativeBuildInputs = [
    pkgs.pkg-config
    pkgs.rustPlatform.bindgenHook
  ];
  buildInputs = [
    pkgs.glib
    pkgs.gtk4
    pkgs.pipewire
    pkgs.dbus
  ];
}
