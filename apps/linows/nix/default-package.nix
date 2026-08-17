# The flake's own build for the evaluating system, used as the default for
# `programs.lookapp.package` in both modules.
#
# `pkgs.callPackage ./package.nix { }` would resolve `rustPlatform` from
# nixpkgs, whose rustc trails the version core needs (see flake.nix), so
# `enable = true` would compile from source and then fail. Going through
# `self.packages` keeps the pinned toolchain and the Cachix hit.
self: pkgs:
self.packages.${pkgs.stdenv.hostPlatform.system}.default
  or (throw "programs.lookapp: no Look package for ${pkgs.stdenv.hostPlatform.system}, set programs.lookapp.package explicitly")
