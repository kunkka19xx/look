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

    # Off on macOS: the cache only ever holds the Linux source build, and the
    # macOS package is a fixed-output fetch of the release zip, so the
    # substituter would sit in nix.conf without ever matching anything.
    cachix = lib.mkOption {
      type = lib.types.bool;
      default = pkgs.stdenv.hostPlatform.isLinux;
      defaultText = lib.literalExpression "pkgs.stdenv.hostPlatform.isLinux";
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
