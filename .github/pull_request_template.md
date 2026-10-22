## Summary

-

## Why

<!-- For a new or large feature, link the issue that was agreed first:
     Closes #123. Features without a prior issue may be asked to open one. -->

## Changes

-

## Testing

Tick what you ran. Delete whole blocks that this PR does not touch.

### Rust core

<!-- Delete this block unless core/** or bridge/ffi/** changed. -->

- [ ] `cargo test --workspace --manifest-path core/Cargo.toml`
- [ ] `cargo test --manifest-path bridge/ffi/Cargo.toml`
- [ ] `cargo fmt --all --manifest-path core/Cargo.toml -- --check`
- [ ] `cargo clippy --workspace --manifest-path core/Cargo.toml -- -D warnings`

### Linux + Windows app (gpui shell, `apps/linows`)

<!-- Delete this block unless apps/linows/** or core/** changed. -->

- [ ] `cargo build --locked --manifest-path apps/linows/Cargo.toml`
- [ ] `cargo test --locked --manifest-path apps/linows/Cargo.toml`
- [ ] `cargo fmt --all --manifest-path apps/linows/Cargo.toml -- --check`
- [ ] `cargo clippy --locked --workspace --all-targets --manifest-path apps/linows/Cargo.toml -- -D warnings`
- [ ] `node --test apps/linows/tests/*.test.js`

### Nix package

<!-- Delete this block unless packaging/nix/**, the flake, or packaging changed. -->

- [ ] `nix build .#default -L`

### macOS app (SwiftUI shell, `apps/macos`)

<!-- Delete this block unless apps/macos/** or bridge/ffi/** changed. -->

- [ ] `xcodebuild -project "apps/macos/look-app.xcodeproj" -scheme "Look" -configuration Debug -destination "platform=macOS" build`
- [ ] `cd apps/macos && swift test`

### By hand

<!-- Delete this block if nothing user-visible changed. -->

- [ ] Ran the app and verified the change (`make app-run`, or `cd apps/linows && cargo run`)
- [ ] Platforms checked:

## Screenshots / Recordings (if UI changed)

### Before

### After

## Risks / Notes

-

## Checklist

<!-- Pre-ticked. Untick anything that is not true of this PR. -->

- [x] I have read and agree to the CLA (CLA.md)
- [x] A new or large feature was discussed in an issue before this PR
- [x] PR title is clear and scoped, with an area prefix (`ui:`, `engine:`, `indexing:`, `ffi:`)
- [x] Docs updated for user-visible changes (`docs/features.md`, `docs/user-guide.md`)
- [x] No secrets or private files included
- [x] Backward compatibility considered
