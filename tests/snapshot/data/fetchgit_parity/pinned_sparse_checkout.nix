# Pinned fetchgit with sparseCheckout = ["src"] and empty hash.
# Tests that the Rust nix-prefetch-git computes the correct hash when
# only a subset of the repository is checked out via git sparse-checkout.
# The sparseCheckout option causes the fetcher to use the git-clone path
# (not tarball) and only materializes the listed paths. This produces a
# different hash than a full checkout. The # pin comment prevents version
# updates; only the empty hash should be filled.
# Upstream reference: nix-prefetch-git --url https://github.com/expipiplus1/update-nix-fetchgit --rev v0.2.1 --sparse-checkout src
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "v0.2.1"; # pin
    hash = "";
    sparseCheckout = [ "src" ];
  };
}
