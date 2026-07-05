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
  libappindicator3-dev pkg-config

cd apps/linows
cargo tauri dev
```

### Arch Linux

```bash
sudo pacman -S --needed \
  base-devel rustup \
  webkit2gtk-4.1 gtk3 libsoup3 glib2 cairo pango \
  gdk-pixbuf2 harfbuzz dbus alsa-lib librsvg openssl pkg-config

rustup default stable
cargo install tauri-cli --version "^2" --locked

cd apps/linows
cargo tauri dev
```

> `base-devel` provides `gcc` / `cc`, without it the Rust build fails with `error: linker 'cc' not found` on a fresh Arch install.

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

The Makefile dispatches to `Makefile.win` on Windows and wraps every target in the vcvars environment:

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

**Dev paths:** in dev mode, Look writes to `%LOCALAPPDATA%\look\look.dev.db` and `%USERPROFILE%\.look.dev.config`. Production builds use `%LOCALAPPDATA%\look\` for both.

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

## Planned: Package Manager Installation

The following installation methods are planned but **not yet available**.
For now, build from source using the instructions above.

### Ubuntu / Debian (.deb)

**Target:** `sudo dpkg -i Look_*.deb` or download from GitHub Releases.

Steps to implement:

1. Generate multi-size icons from `icon.png` (`cargo tauri icon`)
2. Add `bundle` section to `tauri.conf.json` with deb runtime deps
3. Create `.desktop` file at `apps/linows/assets/lookapp.desktop`
4. Create CI workflow (`.github/workflows/release-linux.yml`, ubuntu-22.04 runner)
5. Test: `cargo tauri build` → install .deb on Ubuntu → verify launch + hotkey

### Arch Linux (AUR)

**Target:** `yay -S look-bin`

Steps to implement:

1. Create `apps/linows/packaging/PKGBUILD` (source build from GitHub tarball)
2. `makedepends`: rust, cargo, cargo-tauri, pkg-config, openssl, librsvg
3. `depends`: webkit2gtk-4.1, gtk3, libsoup3, alsa-lib, dbus, xdg-desktop-portal
4. Build step: `cargo tauri build --bundles none` (binary only, pacman handles install)
5. Install: binary → `/usr/bin/`, .desktop → `/usr/share/applications/`, icon → `/usr/share/icons/`
6. Generate `.SRCINFO` and push to AUR git repo
7. Test: `makepkg -si` in clean Arch container

### NixOS (flake)

**Status:** Available now.

```bash
# Run directly
nix run github:kunkka19xx/look?dir=apps/linows

# Install to profile
nix profile install github:kunkka19xx/look?dir=apps/linows

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

> **Note:** The NixOS module requires a NixOS system configuration. Home Manager is not currently supported; use `nix profile install` or the overlay instead. Contributions to add Look to [nixpkgs](https://github.com/NixOS/nixpkgs) or a Home Manager module are welcome.

### AppImage (universal)

**Status:** Available now.

Download `Look_<version>_amd64.AppImage` from GitHub Releases, then `chmod +x && ./Look_*.AppImage`. Built by CI alongside the .deb. For local builds see [Building an AppImage Locally](#building-an-appimage-locally).
