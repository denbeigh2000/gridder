{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix, naersk }:
    let

    in
    (flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ fenix.overlays.default ];
        };

        macPkgs =
          if pkgs.targetPlatform.isDarwin
          then [ pkgs.darwin.apple_sdk.frameworks.SystemConfiguration ]
          else [ ];

        # TODO: rustfmt-nightly
        toolchain = pkgs.fenix.complete.withComponents [
          "cargo"
          "clippy"
          "rust-src"
          "rustc"
        ];

        naersk' = naersk.lib.${system}.override {
          cargo = toolchain;
          rustc = toolchain;
        };

        pkg = naersk'.buildPackage {
          src = ./.;
        };
      in
      {
        packages = {
          gridder = pkg;
          default = pkg;
        };
        devShells.default = pkgs.mkShell {
          packages = [ toolchain pkgs.rust-analyzer-nightly pkgs.libiconv ] ++ macPkgs;
        };
      }));
}
