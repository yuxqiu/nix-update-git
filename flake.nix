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
          nix-update-git =
            let
              runtimeDeps = with pkgs; [
                git
                nix-prefetch-git
              ];
            in
            pkgs.rustPlatform.buildRustPackage {
              pname = cargoToml.package.name;
              version = cargoToml.package.version;
              src = pkgs.lib.cleanSource self;
              cargoLock.lockFile = ./Cargo.lock;

              NIX_UPDATE_GIT_HASH = self.shortRev or "unknown";

              nativeBuildInputs = with pkgs; [ makeWrapper ];

              postInstall = ''
                wrapProgram $out/bin/nix-update-git \
                  --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps}
              '';

              checkFlags = [
                "--skip=test_update_mode_ref"
                "--skip=test_update_mode_inline_ref"
                "--skip=test_flake_input_detects_version_update"
                "--skip=test_flake_input_dotted_form"
                "--skip=test_flake_input_inline_ref_github"
                "--skip=test_flake_input_inline_ref_bare_string"
                "--skip=test_flake_input_inline_ref_no_update"
                "--skip=test_github_fetch_from_github_detects_update"
                "--skip=test_github_fetch_from_github_tag_attribute"
                "--skip=test_github_fetch_from_github_update_mode"
                "--skip=test_github_fetchgit_detects_update"
                "--skip=test_github_builtins_fetch_git"
                "--skip=test_github_fetch_from_github_no_update_when_latest"
                "--skip=test_github_fetch_from_github_pinned"
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
            packages = with pkgs; [
              rustc
              cargo
              rust-analyzer
              cargo-watch
              git
              nix-prefetch-git
            ];
          };
        }
      );

      overlays.default = final: prev: {
        nix-update-git = self.packages.${prev.system}.default;
      };
    };
}
