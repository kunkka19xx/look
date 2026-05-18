# Installing Look on Windows

Look ships as a per-user NSIS installer (`Look_<version>_x64-setup.exe`) attached to each GitHub Release. The installer bundles everything — `lookapp.exe`, `WebView2Loader.dll`, and a WebView2 runtime bootstrapper — so there's nothing else to download. The MSVC C runtime is statically linked, so the app runs on a clean Windows install without the VC++ redistributable.

## One-liner (recommended)

```powershell
iex "& { $(irm https://raw.githubusercontent.com/kunkka19xx/look/main/scripts/windows/install-look.ps1) }"
```

This resolves the latest release, downloads `Look_<version>_x64-setup.exe`, verifies its SHA256 against the published checksums, runs the installer silently into `%LOCALAPPDATA%\Programs\Look`, and launches the app. No admin rights, no PATH mutation.

Pin a version: append `-Version 1.0.0`. Skip the auto-launch at the end: append `-Launch:$false`. Uninstall (silent): append `-Uninstall`.

## Manual install

1. Download `Look_<version>_x64-setup.exe` from the latest [GitHub Release](https://github.com/kunkka19xx/look/releases/latest).
2. Run the installer. It installs to `%LOCALAPPDATA%\Programs\Look` and **does not require administrator rights**.
3. Launch Look from the Start menu or press the global hotkey `Alt+Space`. The hotkey is not user-configurable yet — if it conflicts with another app, remap that app.

### "Windows protected your PC" (SmartScreen)

The installer is currently **unsigned**, so Windows Defender SmartScreen flags it on first run. This is expected.

1. On the SmartScreen dialog, click **More info**.
2. A **Run anyway** button appears below the publisher line. Click it.
3. The installer proceeds normally.

If you're cautious, verify the SHA256 from the release page's `Look-<version>-windows-checksums.txt` before running:

```powershell
Get-FileHash -Algorithm SHA256 .\Look_<version>_x64-setup.exe
```

## Upgrade

Re-run a newer installer. NSIS will replace the previous version in place. Your settings and clipboard history under `%LOCALAPPDATA%\look\` are preserved.

## Uninstall

Settings → Apps → Installed apps → search "Look" → Uninstall. Or run `%LOCALAPPDATA%\Programs\Look\uninstall.exe`.

Removing user data (after uninstall):

```powershell
Remove-Item -Recurse "$env:LOCALAPPDATA\look"
```

## Auto-start

Look offers to register itself for auto-start on first launch. It writes a single entry under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (no scheduled tasks, no services). To disable later, toggle the setting in Look's preferences or delete the registry value.

## Future: `winget install`

A winget manifest will be submitted to `microsoft/winget-pkgs` after the first stable release. Once accepted, installation becomes:

```powershell
winget install kunkka19xx.Look
```

This will also bypass SmartScreen via winget's manifest-hash trust model.

## Verifying you're running the latest

Look shows its version in **Settings → Advanced**. Compare against the latest [GitHub Release](https://github.com/kunkka19xx/look/releases/latest).
