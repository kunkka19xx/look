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
      linuxSystems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      # arm64 only, and the macOS app ships as a signed bundle rather than a
      # source build. Both reasons live in packaging/nix/package-darwin.nix.
      darwinSystems = [ "aarch64-darwin" ];

      forLinuxSystems = nixpkgs.lib.genAttrs linuxSystems;
      forDarwinSystems = nixpkgs.lib.genAttrs darwinSystems;

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
        if pkgs.stdenv.hostPlatform.isDarwin then
          pkgs.callPackage ./packaging/nix/package-darwin.nix { }
        else
          pkgs.callPackage ./packaging/nix/package.nix {
            rustPlatform = rustPlatformFor pkgs;
          };
    in
    {
      packages =
        forLinuxSystems (system: {
          default = lookappFor (pkgsFor system);
        })
        # Plain legacyPackages, not pkgsFor: nothing is compiled on macOS, so
        # the pinned toolchain is beside the point and composing rust-overlay
        # would only slow evaluation down.
        // forDarwinSystems (system: {
          default = lookappFor nixpkgs.legacyPackages.${system};
        });

      # Every module defaults `programs.lookapp.package` to `self.packages`, so
      # `enable = true` gets the pinned toolchain and the cached build on Linux,
      # and the signed release bundle on macOS.
      nixosModules.default = import ./packaging/nix/module.nix self;
      # `environment.systemPackages` and `nix.settings` mean the same thing in
      # nix-darwin, and its activation links the bundle into
      # /Applications/Nix Apps, so the one module serves both.
      darwinModules.default = import ./packaging/nix/module.nix self;
      homeModules.default = import ./packaging/nix/home-manager.nix self;

      # Composed with rust-overlay so `pkgs.lookapp` builds against the pinned
      # toolchain wherever the overlay is applied, not nixpkgs' older rustc.
      overlays.default = nixpkgs.lib.composeExtensions rust-overlay.overlays.default (
        final: _prev: {
          lookapp = lookappFor final;
        }
      );

      # Linux only: the shell carries the GTK, Wayland and Vulkan stack the
      # source build links against, none of which applies to the macOS app.
      devShells = forLinuxSystems (
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
            gtk-layer-shell
            libsoup_3
            glib
            cairo
            pango
            gdk-pixbuf
            harfbuzz
            librsvg
            alsa-lib
            libappindicator-gtk3
            # The gpui port: wgpu, xkbcommon and font-kit link against these,
            # and the Wayland and Vulkan loaders are dlopened at runtime.
            wayland
            vulkan-loader
            libxkbcommon
            libxcb
            fontconfig
            freetype
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
              # The CLI a `run` source block shells out to; the app links rusqlite.
              sqlite
              # gpui/tools: frame capture and the control socket.
              wf-recorder
              ffmpeg
              socat
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
