{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-parts,
    rust-overlay,
    crane,
  } @ inputs:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      perSystem = {system, ...}: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [rust-overlay.overlays.default];
        };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
        src = self;

        nativeBuildInputs = with pkgs; [
          cmake
          pkg-config
        ];
        buildInputs = with pkgs; [
          libopus
          openssl
          sqlite
        ];

        commonArgs = {
          inherit src nativeBuildInputs buildInputs;
          strictDeps = true;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        crate = craneLib.buildPackage (commonArgs // {inherit cargoArtifacts;});

        crate-clippy = craneLib.cargoClippy (
          commonArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "-- --deny warnings --allow unused";
          }
        );

        crate-fmt-check = craneLib.cargoFmt {inherit src;};
      in {
        packages.default = crate;

        checks = {
          inherit crate crate-clippy crate-fmt-check;
        };

        devShells.default = pkgs.mkShell {
          inherit buildInputs;

          nativeBuildInputs = nativeBuildInputs ++ [rustToolchain];

          env = {
            RUST_BACKTRACE = "full";
          };

          shellHook = ''
            # Required for use by RustRover, since it doesn't find the toolchain or stdlib by using the PATH
            # RustRover must then be configured to look inside this symlink for the toolchain
            ln --symbolic --force --no-dereference --verbose "${rustToolchain}" "./.direnv/rust-toolchain"
          '';
        };
      };
    };
}
