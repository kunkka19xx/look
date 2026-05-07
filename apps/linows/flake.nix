{

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs =
    { nixpkgs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              pkg-config
              cargo
              rustc
              cargo-tauri
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
                ]
              }:$LD_LIBRARY_PATH"
            '';
          };

        }
      );
    };
}
