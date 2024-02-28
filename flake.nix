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

  outputs = { self, nixpkgs, rust-overlay, oranda, ... }:
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
            buildFeatures = if stdenv.isLinux then [ "hotplug" ] else [ ];
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
          retries = cfg.client.retries;
        };
        execStart = ''
          ${self.packages.${pkgs.system}.default}/bin/vupdated \
          --config /etc/${daemonName}.toml \
          --key ${cfg.client.apiKey} \
          --server http://${cfg.client.hostname}:${toString cfg.client.port}'';
      in
      {
        options.services.vu-dials.${daemonName} = with types; let
          duration = uniq str;
        in
        {
          enable = mkEnableOption "Enable the VU-1 dials update daemon";
          enableHotplug = mkEnableOption "Enable USB hotplug support for VU-Server";
          logFilter = mkOption
            {
              type = separatedString ",";
              default = "info";
              example = "info,vupdaters=debug";
              description = "`tracing-subscriber` log filtering configuration for vupdated";
            };
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
                retries = mkOption {
                  description = "VU-Server client retry configuration";
                  default = { };

                  type = submodule {
                    options = {
                      initial-backoff = mkOption {
                        type = duration;
                        default = "500ms";
                        example = "500ms";
                        description = "Initial backoff time for retries";
                      };
                      jitter = mkOption {
                        type = uniq float;
                        default = 0.5;
                        example = 0.5;
                        description = "Random jitter factor for retry backoff. When backing off, the duration will be multiplied by a random number between `jitter` and `jitter + 1`";
                      };
                      multiplier = mkOption {
                        type = uniq float;
                        default = 1.5;
                        example = 1.5;
                        description = "Exponential backoff multiplier. When backing off, the previous backoff duration will be multiplied by this number.";
                      };
                      max-backoff = mkOption {
                        type = duration;
                        default = "10m";
                        example = "10m";
                        description = "Maximum backoff time for retries. If a request has not succeeded within this duration, the request will be permanently failed.";
                      };
                    };
                  };
                };
              };
            };
          };
          dials =
            let
              defaultUpdateInterval = "1s";
              defaultDials =
                {
                  "CPU Load" = {
                    index = 0;
                    metric = "cpu-load";
                    update-interval = defaultUpdateInterval;
                  };
                  "Memory Usage" = {
                    index = 1;
                    metric = "mem";
                    update-interval = defaultUpdateInterval;
                  };
                  "CPU Temperature" = {
                    index = 2;
                    metric = "cpu-temp";
                    update-interval = defaultUpdateInterval;
                  };
                  "Swap Usage" = {
                    index = 3;
                    metric = "swap";
                    update-interval = defaultUpdateInterval;
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

              environment.etc."${daemonName}.toml".source = configFile;

              services.vu-dials.server.enable = true;

              systemd.services = {
                ${serverName} = {
                  serviceConfig = {
                    # Ensure that VU-Server runs as the `vudials` user, which
                    # has access to the dialout group.
                    User = userName;
                    DynamicUser = lib.mkForce false;
                  };
                };

                ${daemonName} = {
                  description = "Streacom VU-1 dials update daemon";
                  wantedBy = [ "multi-user.target" ];
                  after = [ serverUnit ];
                  environment = {
                    RUST_LOG = cfg.logFilter;
                  };
                  serviceConfig = {
                    ExecStart = lib.mkDefault execStart;
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
              };
            }
            (mkIf cfg.enableHotplug {

              security.polkit.extraConfig = builtins.readFile ./vupdaters/vupdated-hotplug.rules;

              systemd.services.${daemonName} = {
                serviceConfig = {
                  User = userName;
                  DynamicUser = lib.mkForce false;
                  ExecStart = lib.mkForce ''
                    ${execStart} \
                    --hotplug --hotplug-service ${serverUnit}
                  '';
                };
              };
            })
          ]);
      };
    };
}
