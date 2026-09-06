# Building & Packaging: linows

Build instructions for the Look desktop app (Linux + Windows target via Tauri v2).

## Architecture support

**Released builds target x86_64 (x64) only** on both Linux and Windows.

- ARM64 builds aren't shipped. Windows on ARM (Surface Pro X, Snapdragon X laptops) is still <2% of the install base; users there can run the x64 build under Windows' x64 emulation with a small perf hit. Linux on ARM is rarely a desktop target.
- The workspace `.cargo/config.toml` already declares `+crt-static` for both `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc`, so adding an ARM matrix to the release workflow later is mechanical (rustup target + `cargo tauri build --target`).
- If you have a real ARM machine and want native builds, please open an issue; the project will add an ARM track when there's demand.

---

## Prerequisites

- **Rust** stable toolchain (`rustup`)
- **cargo-tauri** CLI (`cargo install tauri-cli --version "^2" --locked`)
- System libraries (see per-distro sections below)

---

## Build from Source (Development)

### Ubuntu / Debian

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
  libglib2.0-dev libcairo2-dev libpango1.0-dev \
  libgdk-pixbuf-2.0-dev libharfbuzz-dev libdbus-1-dev \
  libasound2-dev librsvg2-dev libssl-dev \
  libappindicator3-dev pkg-config libgtk-layer-shell0

cd apps/linows
cargo tauri dev
```

### Arch Linux

```bash
sudo pacman -S --needed \
  base-devel rustup \
  webkit2gtk-4.1 gtk3 libsoup3 glib2 cairo pango \
  gdk-pixbuf2 harfbuzz dbus alsa-lib librsvg openssl pkg-config \
  gtk-layer-shell

rustup default stable
cargo install tauri-cli --version "^2" --locked

cd apps/linows
cargo tauri dev
```

> `base-devel` provides `gcc` / `cc`, without it the Rust build fails with `error: linker 'cc' not found` on a fresh Arch install.

> `gtk-layer-shell` is a runtime dep, dlopened rather than linked. Without it the app starts and logs `libgtk-layer-shell.so.0 not loadable`, then falls back to a normal toplevel window that a fullscreen window can cover. It only matters under sway / niri / Hyprland; mutter has no `wlr-layer-shell` to use it with.

### NixOS

```bash
nix develop --accept-flake-config ./apps/linows/
cargo tauri dev
```

The `flake.nix` provides all dependencies automatically. Pass `--accept-flake-config` to trust the Cachix substituter, or add `trusted-substituters = https://look.cachix.org` to your `~/.config/nix/nix.conf` to avoid the prompt.

For i3/X11 without compositor:

```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 cargo tauri dev
```

### Home Manager

The flake exports a Home Manager module for declarative user configuration. Add
the input and make sure `inputs` reaches your modules, since Home Manager does
not pass it by default:

```nix
# flake.nix
{
  inputs.look.url = "github:kunkka19xx/look?dir=apps/linows";

  outputs = { nixpkgs, home-manager, ... }@inputs: {
    homeConfigurations."me" = home-manager.lib.homeManagerConfiguration {
      pkgs = nixpkgs.legacyPackages.x86_64-linux;
      extraSpecialArgs = { inherit inputs; };   # required for the import below
      modules = [ ./home.nix ];
    };
  };
}
```

Home Manager as a NixOS module wants `home-manager.extraSpecialArgs = { inherit
inputs; };` instead. Then:

```nix
# home.nix
{ inputs, ... }: {
  imports = [ inputs.look.homeModules.default ];

  programs.lookapp = {
    enable = true;
    theme = "kindle";
    settings = {
      running_apps_placement = "right";
      file_scan_extra_roots = [ "~/Projects" "/mnt/data" ];
      ai_enabled = false;
    };
    aliases = {
      note = [ "Obsidian" "Logseq" ];
      term = [ "Alacritty" "Kitty" ];
    };
  };
}
```

`package` defaults to the flake's own build, so nothing else needs wiring. Set
it to `null` to manage only the config file, for example when the package is
already installed system-wide through `environment.systemPackages` or
`nixosModules.default`.

**Binary cache.** The module deliberately does not touch substituters. Those are
read by the nix daemon, and Home Manager's `nix.settings` only writes
`~/.config/nix/nix.conf`, which the daemon ignores unless you are listed in
`trusted-users`. A flake's own `nixConfig` also applies only when that flake is
the one being built, never when it is an input, so this repo's `nixConfig` does
nothing for you. Put the cache in the flake you actually build:

```nix
# your own flake.nix, top level
nixConfig = {
  extra-substituters = [ "https://look.cachix.org" ];
  extra-trusted-public-keys = [ "look.cachix.org-1:8elPCeSVBzlDZXqIRKBK9GyLIK/Hoe1xiWZF0ir7uX4=" ];
};
```

Nix asks once whether to trust those settings, or pass `--accept-flake-config`.
On NixOS the system-wide equivalent is `programs.lookapp.cachix = true` from
`nixosModules.default`. Elsewhere, `cachix use look` or `/etc/nix/nix.conf`.
Without one of these, Home Manager will build Look from source.

`theme` accepts `catppuccin` (the default), `tokyo-night`, `rose-pine`,
`gruvbox`, `dracula`, `kanagawa`, `kindle`, `liquid` and `custom`. Colours are
derived from the preset at startup, so the module only writes `ui_theme`, plus
the opacity values for `kindle` and `liquid` because those two own them. Use
`custom` to drive every `ui_*` value from `settings` instead.

`settings` keys map directly to `~/.look/config` keys and override values
derived from `theme`. Lists are written as comma-separated values, except
`ignored_patterns_*` and `alias_*`, which Look parses as pipe-separated.
`aliases` is the same thing with the prefix filled in, so declaring
`aliases.note` and `settings.alias_note` together is an error.

Activation merges the managed keys into `~/.look/config` rather than replacing
it: keys you set in Nix win, anything you changed in-app is kept, and keys you
remove from the Nix config are cleaned up on the next rebuild. The file stays
writable so the app can keep saving to it, but Nix wins again on every
activation, so treat Nix as the source of truth for the keys it manages. The
first activation copies the pre-Nix file to `<config>.hm-backup`.

Upgrading from a Look that kept its config at `~/.look.config`: activation
merges into whichever file Look reads, the old one until Look copies it into
`~/.look/` on its next launch, which carries the managed keys across. Nothing
needs doing by hand, and the old file is left where it is.

### Windows

**Prerequisites:**

- Rust stable + cargo-tauri (as above)
- **Visual Studio 2022 Build Tools** (Desktop development with C++ workload, provides both `link.exe` and the Windows SDK)
- WebView2 runtime (ships with Windows 11; for older Win10 the NSIS installer fetches it automatically via the embedded bootstrapper)

**Why VS 2022 Build Tools specifically:** the MSVC linker needs both `link.exe` *and* the Windows SDK. VS 2026 Community ships `link.exe` but no SDK by default, and without it `cargo build` fails with `LNK1104: cannot open file 'msvcrt.lib'`. Same applies to `cargo install tauri-cli`.

**Running cargo under vcvars:** every cargo invocation must run inside a `vcvarsall.bat x64` shell so the linker can find the SDK. The repo provides a wrapper:

```cmd
scripts\windows\with-vcvars.bat cargo tauri dev
scripts\windows\with-vcvars.bat cargo tauri build
```

The Makefile dispatches to `scripts/Makefile.win` on Windows and wraps every target in the vcvars environment:

```bash
make app-run            # cargo tauri dev (hot reload)
make app-run-release    # cargo tauri build (release bundle)
make app-build          # cargo build (debug)
make app-build-release  # cargo build (release)
```

Format and lint are not Make targets; run them directly under vcvars:

```bash
scripts\windows\with-vcvars.bat cargo fmt --manifest-path apps\linows\src-tauri\Cargo.toml -- --check
scripts\windows\with-vcvars.bat cargo clippy --manifest-path apps\linows\src-tauri\Cargo.toml -- -D warnings
```

**Dev paths:** in dev mode, Look writes to `%LOCALAPPDATA%\look\look.dev.db` and `%USERPROFILE%\.look\config.dev`. Production builds use `%LOCALAPPDATA%\look\` for both.

**Hot reload caveats:**

- Tauri dev watches `apps/linows/src-tauri/` only. Changes under `core/engine/` need a touch of any `src-tauri/` file to trigger rebuild.
- Frontend HTML/CSS/JS changes need a manual `Ctrl+R` in the webview; no HMR (`beforeDevCommand` is intentionally empty since `frontendDist` is static).

**Installer output:** `apps\linows\src-tauri\target\release\bundle\nsis\Look_<version>_x64-setup.exe`. The MSVC C runtime is static-linked via the workspace-root `.cargo/config.toml`, so the installer runs on a clean Windows 10/11 install without the VC++ redistributable.

---

## Building an AppImage Locally

Release AppImages are built by CI (`release-linux.yml`) on ubuntu-22.04. To build one locally from your current working tree, for example to test a fix on Fedora or openSUSE before releasing:

```bash
scripts/linux/build-appimage.sh
```

Requires docker. The script builds a `look-appimage-builder` image (ubuntu-22.04 with the same dependency list as CI) and runs `cargo tauri build --bundles appimage` inside it. The cargo target dir and caches live in named docker volumes (`look-appimage-target`, `look-appimage-registry`, `look-appimage-cache`), so incremental rebuilds are fast and host build dirs stay untouched.

Output: `dist/Look_<version>_amd64.AppImage` at the repo root (gitignored).

**Why a container:** Tauri's AppImage bundler runs linuxdeploy, which is itself an AppImage and needs an FHS system. On NixOS it fails outright, and even if forced, the produced binary would embed a `/nix/store` ELF interpreter path and not run on other distros. Building on ubuntu-22.04 also pins the glibc baseline to match releases.

**Running on the target machine:**

```bash
chmod +x Look_*.AppImage
./Look_*.AppImage
```

If FUSE is missing, run with `--appimage-extract-and-run`, or install it (Fedora: `sudo dnf install fuse fuse-libs`).

---

## Runtime Dependencies

| Dependency         | Purpose                         |
| ------------------ | ------------------------------- |
| WebKitGTK 4.1      | WebView rendering               |
| GTK 3              | UI toolkit                      |
| libsoup 3          | HTTP (WebKitGTK dep)            |
| dbus               | System bus                      |
| ALSA (libasound)   | Audio playback (Pomodoro music) |
| xdg-desktop-portal | File picker dialogs             |

---

## Notes

- **Monorepo**: The linows app depends on `core/` crates via path. The full repo checkout is needed to build.
- **WebKitGTK version**: Tauri v2 requires the Soup3 variant (`webkitgtk-4.1`), not the older `webkitgtk-4.0`.
- **NixOS specifics**: Binary wrapping, icon paths in `XDG_DATA_DIRS`, and `wrapGAppsHook` for GTK runtime are handled by the flake.

---

## Package Manager Installation

Prebuilt packages are published on every tagged release. To build from source instead, use the instructions above.

### Ubuntu / Debian (.deb)

**Status:** Available now.

Download `Look_<version>_amd64.deb` from GitHub Releases, then:

```bash
sudo dpkg -i Look_*.deb
sudo apt-get install -f   # pull in any missing runtime deps
```

Built by CI (`.github/workflows/release-linux.yml`) alongside the AppImage.

### Arch Linux (AUR)

**Status:** Available now.

```bash
yay -S look-bin
```

### NixOS (flake)

**Status:** Available now.

```bash
# Run directly
nix run 'github:kunkka19xx/look?dir=apps/linows'

# Install to profile
nix profile install 'github:kunkka19xx/look?dir=apps/linows'

# Build locally
cd apps/linows
nix build .#default
./result/bin/lookapp
```

**Declarative install** (recommended):

```nix
# flake.nix
{
  inputs.look.url = "github:kunkka19xx/look?dir=apps/linows";

  outputs = { nixpkgs, look, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      modules = [
        look.nixosModules.default
        {
          programs.lookapp.enable = true;
          # Binary cache is enabled by default.
          # To disable: programs.lookapp.cachix = false;
        }
      ];
    };
  };
}
```

That's it: the module installs the package and configures the binary cache automatically.

**Other install methods:**

```nix
# Use the package directly
environment.systemPackages = [ inputs.look.packages.${system}.default ];

# Or use the overlay
nixpkgs.overlays = [ inputs.look.overlays.default ];
environment.systemPackages = [ pkgs.lookapp ];
```

For non-NixOS Nix users: `cachix use look` then `nix profile install`.

> **Note:** For user-level declarative installation and configuration, use the Home Manager module described above. The NixOS module is intended for system-level configuration. Contributions to add Look to [nixpkgs](https://github.com/NixOS/nixpkgs) are welcome.

### AppImage (universal)

**Status:** Available now.

Download `Look_<version>_amd64.AppImage` from GitHub Releases, then `chmod +x && ./Look_*.AppImage`. Built by CI alongside the .deb. For local builds see [Building an AppImage Locally](#building-an-appimage-locally).
