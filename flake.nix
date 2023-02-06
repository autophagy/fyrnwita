{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = naersk.lib.${system};
      in
      {
        packages = rec {
          fyrnwita = naersk-lib.buildPackage {
            root = ./.;
            buildInputs = with pkgs; [ pkg-config openssl ];
            doCheck = false;
          };
        };

        defaultPackage = self.packages.${system}.fyrnwita;

        devShell = with pkgs; mkShell {
          buildInputs = [ cargo rustc rustfmt pre-commit rustPackages.clippy dhall openssl pkg-config ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
        };
      });
}
