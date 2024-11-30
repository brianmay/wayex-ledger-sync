{
  description = "Synchronise Wayex and Legdger-CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    crane.url = "github:ipetkov/crane";
    flockenzeit.url = "github:balsoft/flockenzeit";
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      crane,
      flockenzeit,
    }:
    flake-utils.lib.eachSystem
      [
        "x86_64-linux"
        "aarch64-linux"
      ]
      (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };

          rustPlatform = pkgs.rust-bin.stable.latest.default.override {
            targets = [ "wasm32-unknown-unknown" ];
            extensions = [ "rust-src" ];
          };

          craneLib = (crane.mkLib pkgs).overrideToolchain rustPlatform;

          build_env = {
            BUILD_DATE = with flockenzeit.lib.splitSecondsSinceEpoch { } self.lastModified; "${F}T${T}${Z}";
            VCS_REF = "${self.rev or "dirty"}";
          };

          common = {
            src = ./.;
            pname = "wayex-ledger-sync";
            version = "0.1.0";
            # nativeBuildInputs = with pkgs; [ pkg-config ];
            # buildInputs = with pkgs; [
            #   openssl
            #   python3
            #   protobuf
            # ];
            # See https://github.com/ipetkov/crane/issues/414#issuecomment-1860852084
            # for possible work around if this is required in the future.
            # installCargoArtifactsMode = "use-zstd";
          };

          # Build *just* the cargo dependencies, so we can reuse
          # all of that work (e.g. via cachix) when running in CI
          cargoArtifacts = craneLib.buildDepsOnly common;

          # Run clippy (and deny all warnings) on the crate source.
          clippy = craneLib.cargoClippy (
            {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "-- --deny warnings";
            }
            // common
          );

          # Next, we want to run the tests and collect code-coverage, _but only if
          # the clippy checks pass_ so we do not waste any extra cycles.
          coverage = craneLib.cargoTarpaulin ({ cargoArtifacts = clippy; } // common);

          # Build the actual crate itself.
          pkg = craneLib.buildPackage (
            {
              inherit cargoArtifacts;
              doCheck = true;
              # CARGO_LOG = "cargo::core::compiler::fingerprint=info";
            }
            // common
            // build_env
          );

          devShell = pkgs.mkShell {
            packages = [
              pkgs.rust-analyzer
              rustPlatform
            ];
          };

        in
        {
          # Disable coverage checks as broken since Rust 1.77:
          # Mar 27 05:16:41.964 ERROR cargo_tarpaulin::test_loader: Error parsing debug information from binary: An I/O error occurred while reading.
          # Mar 27 05:16:41.964  WARN cargo_tarpaulin::test_loader: Stripping symbol information can prevent tarpaulin from working. If you want to do this pass `--engine=llvm`
          # Mar 27 05:16:41.965 ERROR cargo_tarpaulin: Error while parsing binary or DWARF info.
          # Error: "Error while parsing binary or DWARF info."
          checks = {
            wayex-ledger-sync = clippy;
          };

          devShells.default = devShell;
          packages = {
            default = pkg;
          };
        }
      );
}
