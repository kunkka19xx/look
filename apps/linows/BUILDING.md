# Building & Packaging: linows

Build instructions for the Look desktop app on Linux and Windows: the gpui shell in `gpui/`, binary `lookapp`. The Tauri shell in `src-tauri/` is the backup (`cargo build -p lookapp-tauri`, `cargo tauri dev`); it still needs the WebKitGTK packages the old instructions listed.

## Architecture support

**Released builds target x86_64 (x64) only** on both Linux and Windows.

- ARM64 builds aren't shipped. Windows on ARM (Surface Pro X, Snapdragon X laptops) is still <2% of the install base; users there can run the x64 build under Windows' x64 emulation with a small perf hit. Linux on ARM is rarely a desktop target.
- The workspace `.cargo/config.toml` already declares `+crt-static` for both `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc`, so adding an ARM matrix to the release workflow later is mechanical (rustup target + `cargo build --target`).
- If you have a real ARM machine and want native builds, please open an issue; the project will add an ARM track when there's demand.

---

## Prerequisites

- **Rust** stable toolchain (`rustup`)
- System libraries (see per-distro sections below)

---

## Build from Source (Development)

### Ubuntu / Debian

```bash
sudo apt-get install -y \
  libxkbcommon-dev libxkbcommon-x11-dev libxcb1-dev libxcb-xkb-dev \
  libwayland-dev libasound2-dev libdbus-1-dev libssl-dev \
  libfontconfig1-dev libfreetype-dev pkg-config

cd apps/linows
cargo run
```

### Arch Linux

```bash
sudo pacman -S --needed \
  base-devel rustup \
  libxkbcommon libxkbcommon-x11 libxcb wayland alsa-lib dbus openssl \
  fontconfig freetype2 pkg-config

rustup default stable

cd apps/linows
cargo run
```

> `base-devel` provides `gcc` / `cc`, without it the Rust build fails with `error: linker 'cc' not found` on a fresh Arch install.

> The Vulkan loader, GL and libwayland are opened at runtime, not linked, so they are not build inputs. Without a Vulkan driver wgpu falls back to GL.

### NixOS

```bash
cd apps/linows
nix develop --accept-flake-config
cargo run
```

The `flake.nix` provides all dependencies automatically. Pass `--accept-flake-config` to trust the Cachix substituter, or add `trusted-substituters = https://look.cachix.org` to your `~/.config/nix/nix.conf` to avoid the prompt.

### Home Manager

The flake exports a Home Manager module for declarative user configuration. Add the input and make sure `inputs` reaches your modules, since Home Manager does not pass it by default:

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

Home Manager as a NixOS module wants `home-manager.extraSpecialArgs = { inherit inputs; };` instead. Then:

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

`package` defaults to the flake's own build, so nothing else needs wiring. Set it to `null` to manage only the config file, for example when the package is already installed system-wide through `environment.systemPackages` or `nixosModules.default`.

**Binary cache.** The module deliberately does not touch substituters. Those are read by the nix daemon, and Home Manager's `nix.settings` only writes `~/.config/nix/nix.conf`, which the daemon ignores unless you are listed in `trusted-users`. A flake's own `nixConfig` also applies only when that flake is the one being built, never when it is an input, so this repo's `nixConfig` does nothing for you. Put the cache in the flake you actually build:

```nix
# your own flake.nix, top level
nixConfig = {
  extra-substituters = [ "https://look.cachix.org" ];
  extra-trusted-public-keys = [ "look.cachix.org-1:8elPCeSVBzlDZXqIRKBK9GyLIK/Hoe1xiWZF0ir7uX4=" ];
};
```

Nix asks once whether to trust those settings, or pass `--accept-flake-config`. On NixOS the system-wide equivalent is `programs.lookapp.cachix = true` from `nixosModules.default`. Elsewhere, `cachix use look` or `/etc/nix/nix.conf`. Without one of these, Home Manager will build Look from source.

`theme` accepts `catppuccin`, `tokyo-night`, `rose-pine`, `gruvbox`, `dracula`, `kanagawa` (the default), `kindle`, `liquid` and `custom`. Colours are derived from the preset at startup, so the module only writes `ui_theme`, plus the opacity values for `kindle` and `liquid` because those two own them. Use `custom` to drive every `ui_*` value from `settings` instead. The same names work on macOS: that build spells two of them differently (`tokyoNight`, `rosePine`) and tints `liquid` more lightly, and the module writes whichever spelling and values the installed app understands.

`settings` keys map directly to `~/.look/config` keys and override values derived from `theme`. Lists are written as comma-separated values, except `ignored_patterns_*` and `alias_*`, which Look parses as pipe-separated. `aliases` is the same thing with the prefix filled in, so declaring `aliases.note` and `settings.alias_note` together is an error.

Activation merges the managed keys into `~/.look/config` rather than replacing it: keys you set in Nix win, anything you changed in-app is kept, and keys you remove from the Nix config are cleaned up on the next rebuild. The file stays writable so the app can keep saving to it, but Nix wins again on every activation, so treat Nix as the source of truth for the keys it manages. The first activation copies the pre-Nix file to `<config>.hm-backup`.

Upgrading from a Look that kept its config at `~/.look.config`: activation merges into whichever file Look reads, the old one until Look copies it into `~/.look/` on its next launch, which carries the managed keys across. Nothing needs doing by hand, and the old file is left where it is.

### Windows

**Prerequisites:**

- Rust stable (as above)
- **Visual Studio 2022 Build Tools** (Desktop development with C++ workload, provides both `link.exe` and the Windows SDK)
- NSIS (`winget install NSIS.NSIS`), only to build the installer

**Why VS 2022 Build Tools specifically:** the MSVC linker needs both `link.exe` _and_ the Windows SDK. VS 2026 Community ships `link.exe` but no SDK by default, and without it `cargo build` fails with `LNK1104: cannot open file 'msvcrt.lib'`.

**Running cargo under vcvars:** every cargo invocation must run inside a `vcvarsall.bat x64` shell so the linker can find the SDK. The repo provides a wrapper:

```cmd
scripts\windows\with-vcvars.bat cargo run -p lookapp
scripts\windows\with-vcvars.bat cargo build --release -p lookapp
```

The Makefile dispatches to `scripts/Makefile.win` on Windows and wraps every target in the vcvars environment:

```bash
make app-run            # cargo run (debug)
make app-run-release    # cargo run (release)
make app-build          # cargo build (debug)
make app-build-release  # cargo build (release)
make tauri-run          # the Tauri backup, cargo tauri dev
```

Format and lint are not Make targets; run them directly under vcvars:

```bash
scripts\windows\with-vcvars.bat cargo fmt --manifest-path apps\linows\Cargo.toml -- --check
scripts\windows\with-vcvars.bat cargo clippy --manifest-path apps\linows\Cargo.toml -- -D warnings
```

**Dev paths:** in dev mode, Look writes to `%LOCALAPPDATA%\look\look.dev.db` and `%USERPROFILE%\.look\config.dev`. Production builds use `%LOCALAPPDATA%\look\` for both.

**Installer:** `packaging/windows/look.nsi`, built by `release-windows.yml` with the NSIS on the runner:

```cmd
makensis /DVERSION=0.7.2 /DBINARY=apps\linows\target\release\lookapp.exe packaging\windows\look.nsi
```

Output `Look_<version>_x64-setup.exe`, per user, silent with `/S`. The MSVC C runtime is static-linked via the workspace-root `.cargo/config.toml`, so it runs on a clean Windows 10/11 install without the VC++ redistributable.

---

## Building an AppImage Locally

Release AppImages are built by CI (`release-linux.yml`) on ubuntu-22.04. To build one locally from your current working tree, for example to test a fix on Fedora or openSUSE before releasing:

```bash
scripts/linux/build-appimage.sh
```

Requires docker. The script builds a `look-appimage-builder` image (ubuntu-22.04 with the same dependency list as CI), builds `lookapp` inside it and packs it with `scripts/linux/make-appimage.sh`, the same script CI runs. The cargo target dir and caches live in named docker volumes (`look-appimage-target`, `look-appimage-registry`, `look-appimage-cache`), so incremental rebuilds are fast and host build dirs stay untouched.

Output: `dist/Look_<version>_amd64.AppImage` at the repo root (gitignored).

**Why a container:** linuxdeploy is itself an AppImage and needs an FHS system. On NixOS it fails outright, and even if forced, the produced binary would embed a `/nix/store` ELF interpreter path and not run on other distros. Building on ubuntu-22.04 also pins the glibc baseline to match releases.

**Running on the target machine:**

```bash
chmod +x Look_*.AppImage
./Look_*.AppImage
```

If FUSE is missing, run with `--appimage-extract-and-run`, or install it (Fedora: `sudo dnf install fuse fuse-libs`).

---

## Runtime Dependencies

| Dependency          | Purpose                                       |
| ------------------- | --------------------------------------------- |
| xkbcommon, xcb      | Keyboard and the X11 fallback                 |
| Vulkan loader or GL | Drawing (wgpu picks Vulkan, falls back to GL) |
| libwayland          | Wayland client, opened at runtime             |
| dbus                | System bus                                    |
| ALSA (libasound)    | Audio playback (Pomodoro music)               |
| xdg-desktop-portal  | File picker dialogs                           |

---

## Notes

- **Monorepo**: The linows app depends on `core/` crates via path. The full repo checkout is needed to build.
- **Version**: a tagged release carries the tag (`LOOK_VERSION` at build time). Dev and Nix builds carry `[workspace.package] version` from `apps/linows/Cargo.toml`, so bump it with each release.
- **NixOS specifics**: the runtime libraries are added to the binary's rpath and the host tools to its PATH by the flake.

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

Update to the latest release:

```bash
nix flake update look --flake /path/to/your/flake
sudo nixos-rebuild switch --flake /path/to/your/flake#hostname
```

**Window never shows after a system update?** If Look runs but logs `EGL_BAD_PARAMETER`, your graphics driver is newer than Look's pinned nixpkgs. Make the input follow yours:

```nix
look = {
  url = "github:kunkka19xx/look?dir=apps/linows";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

Look then builds locally instead of coming from Cachix.

> **Note:** For user-level declarative installation and configuration, use the Home Manager module described above. The NixOS module is intended for system-level configuration. Look is also in [nixpkgs](https://github.com/NixOS/nixpkgs) as `lookapp`, Linux only, and that package builds the Tauri shell rather than the gpui one.

### macOS (flake)

**Status:** Available now, Apple Silicon only.

```bash
nix profile install 'github:kunkka19xx/look?dir=apps/linows'
```

The flake lives here because every Nix output does, but this package is not the `linows` build: it installs the macOS app from `apps/macos`. That one is an Xcode/Swift target and xcodebuild needs Xcode, which is not in nixpkgs, so there is no source build to offer. The derivation takes the signed and notarized `Look.app` from GitHub Releases instead, the same bundle `brew install --cask look` ships. Keeping Apple's signature is the point rather than a shortcut: macOS keys the Accessibility and Automation grants to it, so a locally re-signed bundle would drop every granted permission on each rebuild, silently, with the switches in System Settings still showing as on.

It installs as `Applications/Look.app` plus a `bin/lookapp` symlink to the bundle executable, the CLI layout `scripts/install-look.sh` already uses. Started through the symlink the app hands off to the bundle, so `lookapp clipboard` and `lookapp --toggle` work from a profile install.

Declarative with [nix-darwin](https://github.com/nix-darwin/nix-darwin):

```nix
# flake.nix
inputs.look.url = "github:kunkka19xx/look?dir=apps/linows";

# in your darwinSystem modules
look.darwinModules.default
{ programs.lookapp.enable = true; }
```

nix-darwin links the bundle into `/Applications/Nix Apps`, which is what gets Look into Spotlight and into the Accessibility list in System Settings. The [Home Manager module](#home-manager) works unchanged as well, and its `targets.darwin.linkApps` puts the bundle in `~/Applications/Home Manager Apps`.

Three things this does not cover:

- **Intel Macs.** Releases publish one asset per architecture and only the arm64 zip exists so far, so `x86_64-darwin` is left out of the flake rather than shipped broken. It is one entry in `nix/package-darwin.nix` once an Intel release lands.
- **The binary cache.** `programs.lookapp.cachix` defaults to off on macOS: the package is a fixed-output fetch of the release zip, which every substituter already serves by hash.
- **A dev shell.** `nix develop` stays Linux-only; the macOS app builds through Xcode (see the repo-root `Makefile`).

### AppImage (universal)

**Status:** Available now.

Download `Look_<version>_amd64.AppImage` from GitHub Releases, then `chmod +x && ./Look_*.AppImage`. Built by CI alongside the .deb. For local builds see [Building an AppImage Locally](#building-an-appimage-locally).
