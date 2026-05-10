# Building & Packaging — linows

Build instructions for the Look desktop app (Linux + Windows target via Tauri v2).

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
sudo pacman -S \
  webkit2gtk-4.1 gtk3 libsoup3 glib2 cairo pango \
  gdk-pixbuf2 harfbuzz dbus alsa-lib librsvg openssl pkg-config

cd apps/linows
cargo tauri dev
```

### NixOS

```bash
cd apps/linows
nix develop
cargo tauri dev
```

The `flake.nix` provides all dependencies automatically. For i3/X11 without compositor:

```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 cargo tauri dev
```

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
| xclip / wl-copy    | Clipboard file copy             |

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

**Target:** `sudo dpkg -i lookapp_*.deb` or download from GitHub Releases.

Steps to implement:

1. Generate multi-size icons from `icon.png` (`cargo tauri icon`)
2. Add `bundle` section to `tauri.conf.json` with deb runtime deps
3. Create `.desktop` file at `apps/linows/assets/lookapp.desktop`
4. Create CI workflow (`.github/workflows/release-linux.yml`, ubuntu-22.04 runner)
5. Test: `cargo tauri build` → install .deb on Ubuntu → verify launch + hotkey

### Arch Linux (AUR)

**Target:** `yay -S lookapp`

Steps to implement:

1. Create `apps/linows/packaging/PKGBUILD` (source build from GitHub tarball)
2. `makedepends`: rust, cargo, cargo-tauri, pkg-config, openssl, librsvg
3. `depends`: webkit2gtk-4.1, gtk3, libsoup3, alsa-lib, dbus, xdg-desktop-portal, xclip
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

That's it — the module installs the package and configures the binary cache automatically.

**Other install methods:**

```nix
# Use the package directly
environment.systemPackages = [ inputs.look.packages.${system}.default ];

# Or use the overlay
nixpkgs.overlays = [ inputs.look.overlays.default ];
environment.systemPackages = [ pkgs.lookapp ];
```

For non-NixOS Nix users: `cachix use look` then `nix profile install`.

> **Note:** The NixOS module requires a NixOS system configuration. Home Manager is not currently supported — use `nix profile install` or the overlay instead. Contributions to add Look to [nixpkgs](https://github.com/NixOS/nixpkgs) or a Home Manager module are welcome.

### AppImage (universal)

**Target:** Download from GitHub Releases, `chmod +x && ./lookapp_*.AppImage`

Steps to implement:

1. Add `"appimage"` to `bundle.targets` in `tauri.conf.json`
2. Built automatically alongside .deb by `cargo tauri build`
3. CI uploads to GitHub Releases
4. Test: run on a minimal distro (Fedora, openSUSE) to verify bundled deps
