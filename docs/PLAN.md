# nix-update-git — Future Plan

## 1. Architecture and rule improvements

### 1.1 `nix-prefetch-git` fallback strategy

If `nix-prefetch-git` is not available, the tool currently just prints a warning and skips hash updates. Consider:

- Auto-detecting availability at startup and informing the user.
- Providing a `--no-prefetch` flag to explicitly skip hash prefetching (useful in CI where `nix-prefetch-git` might not be installed).
- Supporting `nix hash convert` as an alternative for SRI ↔ nix-base32 conversion.

Besides that, `nix-prefetch-git` already exhibits some incompatibilities:

```nix
pkgs.fetchFromGitHub {
  owner = "arkenfox";
  repo = "user.js";
  rev = "140.1";
  hash = "sha256-TyH2YvWIwpIwFaEvU8ZaKLs7IC1NNAV1pDm/GW5bILs="; # wrong hash reported by nix-prefetch-git
  # hash = "sha256-LPDiiEPOZu5Ah5vCLyCMT3w1uoBhUjyqoPWCOiLVLnw=";
}
```
