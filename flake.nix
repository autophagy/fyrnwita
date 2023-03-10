{
  description = "Ansíne - A lightweight dashboard for home servers";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    naersk.url = "github:nix-community/naersk/master";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs { inherit system; };
          naersk-lib = pkgs.callPackage naersk { };
        in
        rec {
          packages = {
            fyrnwita = naersk-lib.buildPackage {
              root = ./.;
              doCheck = true;
            };
            default = packages.fyrnwita;
          };

          devShell = with pkgs; mkShell {
            buildInputs = [ cargo rustc rustfmt rustPackages.clippy ];
            RUST_SRC_PATH = rustPlatform.rustLibSrc;
          };

          formatter = pkgs.nixpkgs-fmt;
        }) // {
      overlays = {
        fyrnwita = final: prev: { inherit (self.packages.${final.system}) fyrnwita; };
        default = self.overlays.fyrnwita;
      };

      nixosModules = {
        fyrnwita = { pkgs, ... }: {
          nixpkgs.overlays = [ self.overlays.default ];
          imports = [ ./module.nix ];
        };
        default = self.nixosModules.fyrnwita;
      };
    };
}
