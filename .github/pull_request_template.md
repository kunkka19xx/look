## Summary

-

## Why

-

## Changes

-

## Testing

- [ ] `cargo check --workspace --manifest-path core/Cargo.toml`
- [ ] `cargo check --manifest-path bridge/ffi/Cargo.toml`
- [ ] macOS app (if `apps/macos/**` touched): `cd apps/macos/LauncherApp && swift test`
- [ ] macOS app (if `apps/macos/**` touched): `xcodebuild -project "apps/macos/LauncherApp/look-app.xcodeproj" -scheme "Look" -configuration Debug -sdk macosx build`
- [ ] Windows app (if `apps/windows/**` touched): `dotnet test apps/windows/LauncherApp.Tests/LauncherApp.Tests.csproj`
- [ ] Windows app (if `apps/windows/**` touched): `dotnet build apps/windows/LauncherApp/LauncherApp.csproj -c Debug -p:Platform=x64 -r win-x64`
- [ ] Manual verification completed (if UI/behavior changed)

## Screenshots / Recordings (if UI changed)

### Before

### After

## Risks / Notes

-

## Checklist

- [ ] PR title is clear and scoped
- [ ] Docs updated for user-visible changes
- [ ] No secrets or private files included
- [ ] Backward compatibility considered
