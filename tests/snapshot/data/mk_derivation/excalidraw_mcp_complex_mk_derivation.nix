# redact: new
# Modified from petertriho/nix-config/pkgs/excalidraw-mcp/default.nix
{
  lib,
  stdenv,
  fetchFromGitHub,
  fetchPnpmDeps,
  pnpm_10,
  pnpmConfigHook,
  nodejs,
  makeWrapper,
  bun,
  typescript,
}:
stdenv.mkDerivation (finalAttrs: {
  pname = "excalidraw-mcp";
  version = "v0.3.2";

  src = fetchFromGitHub {
    owner = "excalidraw";
    repo = "excalidraw-mcp";
    rev = "";
    hash = "";
  };

  nativeBuildInputs = [
    nodejs
    pnpmConfigHook
    pnpm_10
    makeWrapper
    bun
    typescript
  ];

  pnpmDeps = fetchPnpmDeps {
    inherit (finalAttrs) pname version src;
    pnpm = pnpm_10;
    fetcherVersion = 3;
    hash = "sha256-4ufHadONm+GRMQ0rp8rfF4tFZyBC22BJ50zX9Xz6wJI=";
  };

  buildPhase = ''
    runHook preBuild
    pnpm run build
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    install -Dm755 dist/index.js $out/bin/excalidraw-mcp
    runHook postInstall
  '';

  postFixup = ''
    wrapProgram $out/bin/excalidraw-mcp \
      --prefix PATH : ${lib.makeBinPath [ nodejs ]}
  '';

  meta = with lib; {
    description = "Fast and streamable Excalidraw MCP App server";
    homepage = "https://github.com/excalidraw/excalidraw-mcp";
    license = licenses.mit;
    mainProgram = "excalidraw-mcp";
  };
})
