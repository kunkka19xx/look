# macOS app

This directory is the canonical location for the native macOS launcher shell.

Xcode project and app source now live here:

- `apps/macos/look-app.xcodeproj`
- `apps/macos/look-app/`

Shell responsibilities:

- global hotkey registration
- launcher window lifecycle
- keyboard-first result navigation
- action dispatch (open, reveal in Finder, clipboard history copy/delete)
- settings UI: appearance, indexed paths and exclusions, shortcuts, and the Extend tab (declared sources, connectors, lookbook)
