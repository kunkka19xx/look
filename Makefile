# Top-level dispatcher: includes the OS-specific Makefile from scripts/.
# Run any target the same way on either OS, e.g. `make app-build`.
#
# Force a specific build explicitly (from the repo root, so relative paths
# inside the Makefiles resolve):
#   make -f scripts/Makefile.mac     <target>   - macOS / Xcode
#   make -f scripts/Makefile.win     <target>   - Windows / Tauri (apps/linows/)
#   make -f scripts/Makefile.winui3  <target>   - legacy WinUI3 (apps/windows/, reference-only)

ifeq ($(OS),Windows_NT)
include scripts/Makefile.win
else
include scripts/Makefile.mac
endif
