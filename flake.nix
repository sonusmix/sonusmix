{
  description = "Flake for the Sonusmix project.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchain (
          {
            profile = "minimal";
            components = [
              "rustc"
              "rust-std"
              "cargo"
              "clippy"
            ];
          }
          // (builtins.fromTOML (builtins.readFile ./rust-toolchain.toml)).toolchain
        );

        nativeBuildInputs = with pkgs; [
          pkg-config
          rustPlatform.bindgenHook
        ];

        buildInputs = with pkgs; [
          glib
          gtk4
          pipewire
          dbus
        ];
      in
      {
        devShells.minimal = pkgs.mkShell {
          nativeBuildInputs = nativeBuildInputs ++ [ rustToolchain ];
          inherit buildInputs;
        };

        defaultPackage = pkgs.rustPlatform.buildRustPackage {
          pname = "sonusmix";
          version = "0.1.1";
          doCheck = false;
          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "libspa-0.8.0" = "sha256-R68TkFbzDFA/8Btcar+0omUErLyBMm4fsmQlCvfqR9o=";
            };
          };
          src = pkgs.lib.cleanSource ./.;
          inherit nativeBuildInputs;
          inherit buildInputs;
        };
      }
    );
}
