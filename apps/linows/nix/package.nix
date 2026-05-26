{
  lib,
  stdenv,
  rustPlatform,
  pkg-config,
  wrapGAppsHook3,
  webkitgtk_4_1,
  gtk3,
  libsoup_3,
  glib,
  cairo,
  pango,
  gdk-pixbuf,
  harfbuzz,
  alsa-lib,
  dbus,
  openssl,
  libappindicator-gtk3,
  xdg-utils,
  fontconfig,
  curl,
  procps,
}:

rustPlatform.buildRustPackage {
  pname = "lookapp";
  version = (builtins.fromJSON (builtins.readFile ../src-tauri/tauri.conf.json)).version;

  src = lib.cleanSourceWith {
    src = ../../..;
    filter =
      path: _type:
      let
        basePath = toString ../../..;
        relPath = lib.removePrefix basePath (toString path);
      in
      relPath == "/apps"
      || lib.hasPrefix "/apps/linows" relPath
      || lib.hasPrefix "/core" relPath;
  };

  cargoRoot = "apps/linows/src-tauri";
  cargoHash = "sha256-u6VWNd32uNaBwleCnh/eoFRnBZqDmxyItYCLkdfONXc=";

  buildAndTestSubdir = "apps/linows/src-tauri";

  nativeBuildInputs = [
    pkg-config
    wrapGAppsHook3
  ];

  buildInputs = [
    webkitgtk_4_1
    gtk3
    libsoup_3
    glib
    cairo
    pango
    gdk-pixbuf
    harfbuzz
    alsa-lib
    dbus
    openssl
    libappindicator-gtk3
  ];

  preFixup = ''
    gappsWrapperArgs+=(
      --prefix PATH : ${lib.makeBinPath [
        xdg-utils
        fontconfig
        curl
        procps
        glib
      ]}
    )
  '';

  postInstall = ''
    # Desktop file
    mkdir -p $out/share/applications
    cat > $out/share/applications/lookapp.desktop <<'EOF'
[Desktop Entry]
Name=Look
Comment=Keyboard-first desktop launcher
Exec=lookapp
Icon=look
Type=Application
Categories=Utility;
StartupWMClass=Look
EOF

    # Icons
    for size in 32 128 256; do
      icon="$src/apps/linows/src-tauri/icons/''${size}x''${size}.png"
      if [ -f "$icon" ]; then
        mkdir -p $out/share/icons/hicolor/''${size}x''${size}/apps
        cp "$icon" $out/share/icons/hicolor/''${size}x''${size}/apps/look.png
      fi
    done
  '';

  meta = {
    description = "Keyboard-first desktop launcher";
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
    mainProgram = "lookapp";
  };
}
