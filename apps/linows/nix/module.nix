self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.lookapp;
in
{
  options.programs.lookapp = {
    enable = lib.mkEnableOption "Look, a keyboard-first desktop launcher";

    package = lib.mkOption {
      type = lib.types.package;
      default = import ./default-package.nix self pkgs;
      defaultText = lib.literalExpression "inputs.look.packages.\${pkgs.stdenv.hostPlatform.system}.default";
      description = "The Look package to install.";
    };

    cachix = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to add the Look binary cache (look.cachix.org) for pre-built binaries.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    nix.settings = lib.mkIf cfg.cachix {
      substituters = [ "https://look.cachix.org" ];
      trusted-public-keys = [ "look.cachix.org-1:8elPCeSVBzlDZXqIRKBK9GyLIK/Hoe1xiWZF0ir7uX4=" ];
    };
  };
}
