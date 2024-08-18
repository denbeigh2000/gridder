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
      module = import ./nixos/module.nix;
    in
    {
      nixosModules = {
        default = module;
        gridder = module;
      };
    } //
    (flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ fenix.overlays.default ];
        };

        macPkgs = with pkgs; (lib.optionals
          targetPlatform.isDarwin
          [ darwin.apple_sdk.frameworks.SystemConfiguration libiconv ]);

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
          packages = [ toolchain pkgs.rust-analyzer-nightly ] ++ macPkgs;
        };
      }));
}
