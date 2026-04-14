{
  src = pkgs.fetchFromGitHub {
    owner = "arkenfox";
    repo = "user.js";
    rev = "140.0"; # pin
    hash = "";
  };
}
