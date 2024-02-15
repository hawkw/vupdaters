{
  description = "Command line tools and daemons for controlling Streacom VU-1 dials";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    oranda = {
      url = "github:axodotdev/oranda";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    vu-server = {
      url = "github:hawkw/vu-server-flake";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay, oranda, vu-server, ... }:
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
            function pkgs);
      defaultApiKey = "cTpAWYuRpA2zx75Yh961Cg";
      daemonName = "vupdated";
    in
    {
      packages = forAllSystems (pkgs: with pkgs; let
        # use the Rust toolchain specified in the project's rust-toolchain.toml
        rustToolchain =
          let
            file = pkgsBuildHost.rust-bin.fromRustupToolchainFile
              ./rust-toolchain.toml;
          in
          file.override {
            extensions = [
              "rust-src" # for rust-analyzer
            ];
          };

        rustPlatform = makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        src = nix-gitignore.gitignoreSource [ ] ./.;

      in
      {
        default =
          let cargoTOML = lib.importTOML "${src}/vupdaters/Cargo.toml";
          in rustPlatform.buildRustPackage {
            pname = cargoTOML.package.name;
            version = cargoTOML.package.version;
            buildInputs =
              if stdenv.isLinux then [
                udev.dev
              ] else [ ];
            nativeBuildInputs = if stdenv.isLinux then [ pkg-config ] else [ ];
            inherit src;
            cargoLock = { lockFile = "${src}/Cargo.lock"; };
            # PKG_CONFIG_PATH =
            #   if stdenv.isLinux then lib.makeLibraryPath [ pkgs.udev.dev ] else "";
            meta = {
              inherit (cargoTOML.package) description homepage license;
              maintainers = cargoTOML.package.authors;
            };
          };
      });
      devShells = forAllSystems (pkgs: with pkgs; {
        default = mkShell {
          VU_DIALS_API_KEY = defaultApiKey;
          nativeBuildInputs = [ self.packages.${system}.default.nativeBuildInputs ];
          buildInputs = [
            self.packages.${system}.default.buildInputs
            oranda.packages.${system}.default
          ];
        };
      });
      apps = forAllSystems
        (pkgs: with pkgs; {
          dialctl =
            {
              type = "app";
              program = "${self.packages.${system}.default}/bin/dialctl";
            };
          ${daemonName} = {
            type = "app";
            program = "${self.packages.${system}.default}/bin/${daemonName}";
          };
          default = self.apps.${system}.dialctl;
        });
      nixosModules.default = { config, lib, pkgs, ... }: with lib; let
        cfg = config.services.vu-dials.vupdated;
        serverCfg = config.services.vu-dials.server.server;
        serverName = "VU-Server";
        serverUnit = "${serverName}.service";
        userName = "vudials";

        configFormat = pkgs.formats.toml { };
        configFile = configFormat.generate "${daemonName}.toml" {
          dials = cfg.dials;
        };
      in
      {
        options.services.vu-dials.${daemonName} = with types; {
          enable = mkEnableOption "Enable the VU-1 dials update daemon";
          enableHotplug = mkEnableOption "Enable USB hotplug support for VU-Server";

          client = mkOption {
            description = "Configuration for the VU-Server HTTP client.";
            default = { };

            type = submodule {
              options = {
                hostname = mkOption {
                  type = uniq str;
                  default = serverCfg.hostname;
                  example = "localhost";
                  description = "The server's hostname. Probably this should be localhost.";
                };
                port = mkOption {
                  type = uniq port;
                  default = serverCfg.port;
                  example = 5340;
                  description = "The server's HTTP port.";
                };
                apiKey = mkOption {
                  type = uniq string;
                  default = defaultApiKey;
                  example = defaultApiKey;
                  description = "API key to use when communicating with the VU-1 HTTP server";
                };
              };
            };
          };
          dials =
            let
              defaultUpdateInterval = {
                secs = 1;
                nanos = 0;
              };
              defaultDials =
                {
                  "CPU Load" = {
                    index = 0;
                    metric = "cpu-load";
                    update_interval = defaultUpdateInterval;
                  };
                  "Memory Usage" = {
                    index = 1;
                    metric = "mem";
                    update_interval = defaultUpdateInterval;
                  };
                  "CPU Temperature" = {
                    index = 2;
                    metric = "cpu-temp";
                    update_interval = defaultUpdateInterval;
                  };
                  "Swap Usage" = {
                    index = 3;
                    metric = "swap";
                    update_interval = defaultUpdateInterval;
                  };
                };
            in
            mkOption {
              description = "Configuration for the VU-1 dials. This attrset is used to generate the vupdated TOML config file.";
              type = configFormat.type;
              default = defaultDials;
              example = defaultDials;
            };
        };

        config = mkIf cfg.enable
          (mkMerge [
            {
              environment.etc."${daemonName}.toml".source = configFile;
              services.vu-dials.server.enable = true;
              systemd.services.${daemonName} = {
                description = "Streacom VU-1 dials update daemon";
                wantedBy = [ "multi-user.target" ];
                after = [ serverUnit ];
                requisite = [ serverUnit ];
                script = ''
                  ${daemonName} \
                  --config /etc/${daemonName}.toml \
                  --key ${cfg.client.apiKey} \
                  --server http://${cfg.client.hostname}:${toString cfg.client.port}
                '';
                path = [ self.packages.${pkgs.system}.default ];
                serviceConfig = {
                  Restart = "on-failure";
                  RestartSec = "5s";
                  DynamicUser = lib.mkDefault true;
                  RuntimeDirectory = daemonName;
                  RuntimeDirectoryMode = "0755";
                  StateDirectory = daemonName;
                  StateDirectoryMode = "0755";
                  CacheDirectory = daemonName;
                  CacheDirectoryMode = "0750";
                };
              };
            }
            (mkIf cfg.enableHotplug {
              users = {
                users.${userName} = {
                  isSystemUser = true;
                  isNormalUser = false;
                  home = "/home/${userName}";
                  createHome = true;
                  group = userName;
                  extraGroups = [ "dialout" ];
                };
                groups.${userName} = { };
              };

              systemd.services = {
                ${serverName} = {
                  serviceConfig = {
                    User = userName;
                    DynamicUser = lib.mkForce false;
                  };
                };
                ${daemonName} = {
                  serviceConfig = {
                    User = userName;
                    DynamicUser = lib.mkForce false;
                  };
                  scriptArgs = "--hotplug --hotplug-service ${serverUnit}";
                };
              };
            })
          ]);
      };
    };
}
