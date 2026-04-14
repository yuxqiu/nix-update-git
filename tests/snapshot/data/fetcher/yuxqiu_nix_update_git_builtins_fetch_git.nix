# redact: new
{
  src = builtins.fetchGit {
    url = "https://github.com/yuxqiu/nix-update-git";
    ref = "v0.1.0";
  };
}
