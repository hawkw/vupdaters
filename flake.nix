{
  description = "Command line tools and daemons for controlling Streacom VU-1 dials";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      overlays = [ (import rust-overlay) ];
      forAllSystems = function:
        nixpkgs.lib.genAttrs systems
          (system:
            let
              pkgs = import nixpkgs {
                inherit system overlays;
              };
            in
            function { inherit system pkgs; });

      src = ./.;
    in
    {
      packages = forAllSystems ({ pkgs, ... }: with pkgs; let
        # use the Rust toolchain specified in the project's rust-toolchain.toml
        rustToolchain = pkgsBuildHost.rust-bin.fromRustupToolchainFile
          ./rust-toolchain.toml;

        rustPlatform = makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };
      in
      {
        default =
          let cargoTOML = lib.importTOML "${src}/vupdaters/Cargo.toml";
          in rustPlatform.buildRustPackage rec {
            pname = cargoTOML.package.name;
            version = cargoTOML.package.version;

            inherit src;

            cargoLock = { lockFile = "${src}/Cargo.lock"; };

            meta = {
              inherit (cargoTOML.package) description homepage license;
              maintainers = cargoTOML.package.authors;
            };
          };
      });
      devShells = forAllSystems ({ pkgs, system }: {
        default = pkgs.mkShell {
          buildInputs = [ self.packages.${system}.default.buildInputs ];
        };
      });
      apps = forAllSystems
        ({ system, ... }: {
          dialctl =
            {
              type = "app";
              program = "${self.packages.${system}.default}/bin/dialctl";
            };
          vupdated = {
            type = "app";
            program = "${self.packages.${system}.default}/bin/vupdated";
          };
          default = self.apps.${system}.dialctl;
        });
    };
}
