## Summary

-

## Why

-

## Changes

-

## Testing

- `core`
- [ ] `cargo check --workspace --manifest-path core/Cargo.toml`
- [ ] `cargo test --workspace --manifest-path core/Cargo.toml`

- `bridge/ffi`
- [ ] `cargo check --manifest-path bridge/ffi/Cargo.toml`
- [ ] `cargo test --manifest-path bridge/ffi/Cargo.toml`

- `apps/linows`
- [ ] `cargo check --manifest-path apps/linows/src-tauri/Cargo.toml`
- [ ] `cargo test --locked --manifest-path apps/linows/src-tauri/Cargo.toml`
- [ ] `cargo fmt --all --manifest-path apps/linows/src-tauri/Cargo.toml -- --check`
- [ ] `cargo clippy --locked --manifest-path apps/linows/src-tauri/Cargo.toml -- -D warnings`
- [ ] macOS app (if `apps/macos/**` touched): `cd apps/macos/LauncherApp && swift test`
- [ ] macOS app (if `apps/macos/**` touched): `xcodebuild -project "apps/macos/LauncherApp/look-app.xcodeproj" -scheme "Look" -configuration Debug -sdk macosx build`
- [ ] Tauri release bundle (if packaging / release flow changed): `cd apps/linows && cargo tauri build`
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
