{
  description = "Local-first interactive CLI flag discovery and shell editing";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSystem = function:
        nixpkgs.lib.genAttrs systems (system: function {
          inherit system;
          pkgs = import nixpkgs { inherit system; };
        });
    in {
      packages = forEachSystem ({ system, pkgs }:
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "flagpick";
            version = "0.1.0";
            src = self;

            cargoLock.lockFile = ./Cargo.lock;
          };
        });

      apps = forEachSystem ({ system, ... }:
        {
          default = {
            type = "app";
            program = "${self.packages.${system}.default}/bin/flagpick";
          };
        });
    };
}
