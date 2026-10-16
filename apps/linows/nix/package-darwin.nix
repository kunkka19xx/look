# The macOS app, installed from the release zip rather than built here.
#
# apps/macos is an Xcode/Swift target, so there is no source build to offer:
# xcodebuild needs Xcode, which is not in nixpkgs and cannot be redistributed.
# The published zip is also the only build carrying the Developer ID signature
# and the stapled notarization ticket, and TCC keys the Accessibility and
# Automation grants to that signature, so a locally re-signed bundle would void
# every grant on each rebuild.
{
  lib,
  stdenvNoCC,
  fetchurl,
  unzip,
}:

let
  # Bumped together with the hashes below; the two are only correct as a pair.
  # Not read from ../Cargo.toml: that version moves at release prep, before the
  # zip exists, which would break `nix build` on main.
  version = "0.7.2";

  # One asset per architecture (scripts/release-arch.sh), arm64 keeping the
  # historical name. Both lines are rewritten by scripts/nix-bump-darwin.sh,
  # which .github/workflows/update-nix-darwin.yml runs after a release, so
  # edit them with that script rather than by hand.
  #
  # x86_64-darwin is absent rather than broken: the release workflow has an
  # Intel matrix entry, but no Look-<version>-macOS-x86_64.zip is published
  # yet, and the arm64 zip is thin so Rosetta cannot stand in. The bump script
  # exits 3 when it spots such an asset; add the entry here then.
  sources = {
    aarch64-darwin = {
      url = "https://github.com/kunkka19xx/look/releases/download/v${version}/Look-${version}-macOS.zip";
      hash = "sha256-h8DlDos4YfBnR1cfompTM0YgDUrzcZQHhUVEg1zgR50=";
    };
  };

  inherit (stdenvNoCC.hostPlatform) system;
in
stdenvNoCC.mkDerivation {
  pname = "lookapp";
  inherit version;

  src = fetchurl (sources.${system} or (throw "lookapp: no macOS release asset for ${system}"));

  nativeBuildInputs = [ unzip ];

  # The zip holds Look.app at the top level and nothing beside it.
  sourceRoot = ".";

  # Signed and notarized: stripping or rewriting anything inside the bundle
  # breaks the seal and macOS then refuses to launch it.
  dontFixup = true;

  installPhase = ''
    runHook preInstall

    mkdir -p $out/Applications $out/bin
    cp -R Look.app $out/Applications/

    # The CLI is a symlink to the bundle executable, the way
    # scripts/install-look.sh installs it: started through the link, the app
    # sees it is outside its bundle and hands off to the real one (LaunchModes
    # in apps/macos/LauncherApp/look-app/Support).
    #
    # The executable name is an assumption about the release artifact, and
    # `ln -s` would happily create a dangling link if it ever changed, leaving
    # a derivation that builds and a `lookapp` that does nothing.
    if [ ! -x $out/Applications/Look.app/Contents/MacOS/Look ]; then
      echo "Look.app has no executable at Contents/MacOS/Look" >&2
      exit 1
    fi
    ln -s $out/Applications/Look.app/Contents/MacOS/Look $out/bin/lookapp

    runHook postInstall
  '';

  meta = {
    description = "Keyboard-first, local-first desktop launcher";
    homepage = "https://github.com/kunkka19xx/look";
    changelog = "https://github.com/kunkka19xx/look/releases/tag/v${version}";
    license = lib.licenses.gpl3Plus;
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
    platforms = lib.attrNames sources;
    mainProgram = "lookapp";
  };
}
