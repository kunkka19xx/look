# Top-level dispatcher: includes the OS-specific Makefile.
# Run any target the same way on either OS, e.g. `make app-build`.
# To force a specific one explicitly: `make -f Makefile.mac <target>` or `make -f Makefile.win <target>`.

ifeq ($(OS),Windows_NT)
include Makefile.win
else
include Makefile.mac
endif
