{
  description = "Dev shells for the Sonusmix project.";

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

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay)];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
          rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchain (
            {
              profile = "minimal";
              components = [ "rustc" "rust-std" "cargo" "clippy" ];
            } // (builtins.fromTOML (builtins.readFile ./rust-toolchain.toml)).toolchain
          );
        in
        with pkgs;
        {
          devShells.minimal = mkShell {
            nativeBuildInputs = [
              rustToolchain
              rustPlatform.bindgenHook
              pkg-config
            ];
            buildInputs = [
              gtk4
              pipewire
              dbus
            ];
          };
        }
      );
}
