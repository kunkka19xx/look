# Top-level dispatcher: includes the OS-specific Makefile.
# Run any target the same way on either OS, e.g. `make app-build`.
#
# Force a specific build explicitly:
#   make -f Makefile.mac     <target>   - macOS / Xcode
#   make -f Makefile.win     <target>   - Windows / Tauri (apps/linows/)
#   make -f Makefile.winui3  <target>   - legacy WinUI3 (apps/windows/, reference-only)

ifeq ($(OS),Windows_NT)
include Makefile.win
else
include Makefile.mac
endif
