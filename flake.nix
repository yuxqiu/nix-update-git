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
              ];
            in
            pkgs.rustPlatform.buildRustPackage {
              pname = cargoToml.package.name;
              version = cargoToml.package.version;
              src = pkgs.lib.cleanSource self;
              cargoLock.lockFile = ./Cargo.lock;

              NIX_UPDATE_GIT_HASH = self.shortRev or "unknown";

              nativeBuildInputs = with pkgs; [
                makeWrapper
                git
              ];

              postInstall = ''
                wrapProgram $out/bin/nix-update-git \
                  --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps}
              '';

              cargoTestFlags = [ "--no-default-features" ];

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
            ];
          };
        }
      );

      overlays.default = final: prev: {
        nix-update-git = self.packages.${prev.system}.default;
      };
    };
}
