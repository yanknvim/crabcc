{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [
          (import rust-overlay)
        ];
        pkgs = import nixpkgs {
          system = system;
          overlays = overlays;
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            pkgsCross.riscv64-embedded.riscv-pk
          ];

          nativeBuildInputs = with pkgs; [
            cargo-make
            pkgsCross.riscv64-embedded.buildPackages.gcc
            pkgsCross.riscv64-embedded.buildPackages.gdb

            dtc
            spike

            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" ];
              targets = [
                "riscv64gc-unknown-none-elf"
              ];
            })

            perf
          ];
        };
      }
    );
}
