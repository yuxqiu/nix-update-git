{
  description = "Update git references in Nix flake files and Nix expressions";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      pkgsFor = system: import nixpkgs { inherit system; };
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          nix-update-git = pkgs.rustPlatform.buildRustPackage {
            pname = cargoToml.package.name;
            version = cargoToml.package.version;
            src = pkgs.lib.cleanSource self;
            cargoLock.lockFile = ./Cargo.lock;

            NIX_UPDATE_GIT_HASH = self.shortRev or "unknown";

            nativeBuildInputs = with pkgs; [ pkg-config ];
            buildInputs =
              with pkgs;
              [ openssl ]
              ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
                darwin.apple_sdk.frameworks.Security
              ];

            checkFlags = [
              "--skip=test_update_mode_ref"
              "--skip=test_update_mode_inline_ref"
              "--skip=test_flake_input_detects_version_update"
              "--skip=test_flake_input_dotted_form"
              "--skip=test_flake_input_inline_ref_github"
              "--skip=test_flake_input_inline_ref_bare_string"
              "--skip=test_flake_input_inline_ref_no_update"
            ];

            meta = with pkgs.lib; {
              description = "Update git references in Nix flake files and Nix expressions";
              homepage = "https://github.com/yuxqiu/nix-update-git";
              license = licenses.mit;
              mainProgram = "nix-update-git";
            };
          };
          default = self.packages.${system}.nix-update-git;
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          default = pkgs.mkShell {
            packages =
              with pkgs;
              [
                rustc
                cargo
                rust-analyzer
                cargo-watch
                pkg-config
                openssl
              ]
              ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
                darwin.apple_sdk.frameworks.Security
              ];
          };
        }
      );

      overlays.default = final: prev: {
        nix-update-git = self.packages.${prev.system}.default;
      };
    };
}
