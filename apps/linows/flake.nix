{
  nixConfig = {
    extra-substituters = [ "https://look.cachix.org" ];
    extra-trusted-public-keys = [ "look.cachix.org-1:8elPCeSVBzlDZXqIRKBK9GyLIK/Hoe1xiWZF0ir7uX4=" ];
  };

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;

      # nixpkgs' default rustc lags behind. libsqlite3-sys (via rusqlite 0.40)
      # uses cfg_select!, stable only since Rust 1.95, so the default toolchain
      # fails to build it. Every consumer pins the same version through here.
      rustVersion = "1.95.0";

      pkgsFor =
        system:
        import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

      # `minimal` (cargo + rustc + rust-std) rather than `default`: building
      # never invokes clippy or rustfmt, and `default` also drags in rust-docs,
      # which is 632 MiB unpacked. The devShell keeps `default` because
      # contributors do want those tools.
      rustPlatformFor =
        pkgs:
        let
          toolchain = pkgs.rust-bin.stable.${rustVersion}.minimal;
        in
        pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };

      lookappFor =
        pkgs:
        pkgs.callPackage ./nix/package.nix {
          rustPlatform = rustPlatformFor pkgs;
        };
    in
    {
      packages = forAllSystems (system: {
        default = lookappFor (pkgsFor system);
      });

      # Both modules default `programs.lookapp.package` to `self.packages`, so
      # `enable = true` gets the pinned toolchain and the cached build.
      nixosModules.default = import ./nix/module.nix self;
      homeModules.default = import ./nix/home-manager.nix self;

      # Composed with rust-overlay so `pkgs.lookapp` builds against the pinned
      # toolchain wherever the overlay is applied, not nixpkgs' older rustc.
      overlays.default = nixpkgs.lib.composeExtensions rust-overlay.overlays.default (
        final: _prev: {
          lookapp = lookappFor final;
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
          # Needed both to link against and to find at runtime, so the list is
          # shared between buildInputs and LD_LIBRARY_PATH.
          runtimeLibs = with pkgs; [
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
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              pkg-config
              rust-bin.stable.${rustVersion}.default
              cargo-tauri
              xdg-desktop-portal
              xdg-desktop-portal-gtk
              prettier
            ];

            buildInputs = runtimeLibs;

            shellHook = ''
              export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath runtimeLibs}:$LD_LIBRARY_PATH"
              export GSETTINGS_SCHEMA_DIR="${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}/glib-2.0/schemas''${GSETTINGS_SCHEMA_DIR:+:$GSETTINGS_SCHEMA_DIR}"
            '';
          };
        }
      );
    };
}
