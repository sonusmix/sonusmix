{ pkgs ? import <nixpkgs> { } }:
pkgs.rustPlatform.buildRustPackage rec {
  pname = "sonusmix";
  version = "0.1.0";
  cargoLock = {
    lockFile = ./Cargo.lock;
    outputHashes = {
      "libspa-0.8.0" = "sha256-R68TkFbzDFA/8Btcar+0omUErLyBMm4fsmQlCvfqR9o=";
    };
  };
  src = pkgs.lib.cleanSource ./.;
}
