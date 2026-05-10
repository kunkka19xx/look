{

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.callPackage ./nix/package.nix { };
        }
      );

      nixosModules.default = import ./nix/module.nix;

      overlays.default = final: _prev: {
        lookapp = final.callPackage ./nix/package.nix { };
      };

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          rustToolchain = pkgs.rust-bin.stable."1.95.0".default;
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              pkg-config
              rustToolchain
              cargo-tauri
              xdg-desktop-portal
              xdg-desktop-portal-gtk
            ];

            buildInputs = with pkgs; [
              dbus
              openssl
              webkitgtk_4_1
              gtk3
              libsoup_3
              glib
              cairo
              pango
              gdk-pixbuf
              harfbuzz
              librsvg
              alsa-lib
              libappindicator-gtk3
            ];

            shellHook = ''
              export LD_LIBRARY_PATH="${
                pkgs.lib.makeLibraryPath [
                  pkgs.dbus
                  pkgs.openssl
                  pkgs.webkitgtk_4_1
                  pkgs.gtk3
                  pkgs.libsoup_3
                  pkgs.glib
                  pkgs.cairo
                  pkgs.pango
                  pkgs.gdk-pixbuf
                  pkgs.harfbuzz
                  pkgs.librsvg
                  pkgs.alsa-lib
                  pkgs.libappindicator-gtk3
                ]
              }:$LD_LIBRARY_PATH"
              export GSETTINGS_SCHEMA_DIR="${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}/glib-2.0/schemas''${GSETTINGS_SCHEMA_DIR:+:$GSETTINGS_SCHEMA_DIR}"
            '';
          };

        }
      );
    };
}
