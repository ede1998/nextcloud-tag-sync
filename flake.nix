{
  description = "Synchronize Nextcloud tags with local filesystem XDG tags";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    home-manager.url = "github:nix-community/home-manager";
    home-manager.inputs.nixpkgs.follows = "nixpkgs";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    naersk.url = "github:nix-community/naersk";
  };

  outputs =
    inputs@{ self, ... }:
    let
      # Systems that can run tests:
      supportedSystems = [
        "aarch64-linux"
        "i686-linux"
        "x86_64-linux"
      ];

      # Function to generate a set based on supported systems:
      forAllSystems = inputs.nixpkgs.lib.genAttrs supportedSystems;

      # Attribute set of nixpkgs for each system:
      nixpkgsFor = forAllSystems (
        system:
        import inputs.nixpkgs {
          inherit system;
          overlays = [ inputs.rust-overlay.overlays.default ];
        }
      );

      selectRustToolchain =
        system:
        nixpkgsFor.${system}.rust-bin.selectLatestNightlyWith (
          toolchain:
          toolchain.default.override {
            extensions = [
              "rust-src"
              "rust-analyzer"
            ];
          }
        );
    in
    {
      homeManagerModules = {
        default = self.homeManagerModules.nextcloud-tag-sync;
        nextcloud-tag-sync = import ./home-manager-module self;
      };

      packages = forAllSystems (
        system:
        let
          rust-toolchain = selectRustToolchain system;
          naersk' = nixpkgsFor.${system}.callPackage inputs.naersk {
            cargo = rust-toolchain;
            rustc = rust-toolchain;
          };
          nextcloud-tag-sync = naersk'.buildPackage {
            src = ./.;
          };
        in
        {
          default = nextcloud-tag-sync;
        }
      );

      checks = forAllSystems (system: {
        default = nixpkgsFor.${system}.callPackage ./test/basic.nix {
          home-manager-module = inputs.home-manager.nixosModules.home-manager;
          plasma-module = self.homeManagerModules.plasma-manager;
        };
      });

      devShells = forAllSystems (system: {
        default = nixpkgsFor.${system}.mkShell {
          buildInputs = with nixpkgsFor.${system}; [
            cargo-fuzz
            cargo-llvm-cov
            nixfmt-rfc-style
            openssl
            pkg-config
            (selectRustToolchain system)
          ];
        };
      });
    };
}
