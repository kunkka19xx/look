SHELL := /bin/bash
.DEFAULT_GOAL := help

XCODE_PROJECT := apps/macos/LauncherApp/look-app.xcodeproj
XCODE_SCHEME := Look
XCODE_CONFIG := Debug
XCODE_DERIVED_DATA := .build/xcode

APP_ID := noah-code.Look
APP_PROCESS := Look
APP_BUNDLE := $(XCODE_DERIVED_DATA)/Build/Products/$(XCODE_CONFIG)/Look.app
APP_DEBUG_DYLIB := $(APP_BUNDLE)/Contents/MacOS/Look.debug.dylib
LSREGISTER := /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister

DEV_APP_NAME ?= Look Dev
DEV_APP_ID ?= noah-code.Look.Dev
DEV_APP_INSTALL_BUNDLE := /Applications/$(DEV_APP_NAME).app
LOG_PREDICATE ?= subsystem == "noah-code.Look"

REAL_DB_PATH := $(HOME)/Library/Application Support/look/look.db
DEV_CONFIG_PATH ?= $(HOME)/.look.dev.config
DEV_OPEN_ENV = env -u LOOK_DB_PATH LOOK_CONFIG_PATH="$(DEV_CONFIG_PATH)" LOOK_DEV_HINT=1 LOOK_LOG_LEVEL=debug LOOK_UI_DEBUG_EVENTS=1

.PHONY: help core-check ffi-check app-build app-ensure-bundle app-stop app-run app-open app-install-dev app-run-dev app-uninstall-dev app-logs app-logs-all symbols db-path db-status db-shell db-reset db-refresh

help:
	@printf "look developer tasks\n\n"
	@printf "build/check\n"
	@printf "  make core-check   - cargo check core workspace\n"
	@printf "  make ffi-check    - cargo check ffi crate\n"
	@printf "  make app-build    - xcodebuild app (includes rust ffi build/link)\n"
	@printf "  make symbols      - verify ffi symbols in app binary\n\n"
	@printf "run app\n"
	@printf "  make app-stop     - stop running Look app process\n"
	@printf "  make app-run      - stop running app, then open local app with dev config\n"
	@printf "  make app-open     - open built app bundle\n"
	@printf "  make app-install-dev - install side-by-side /Applications/$(DEV_APP_NAME).app\n"
	@printf "  make app-run-dev  - install+open side-by-side dev app with dev config\n"
	@printf "  make app-uninstall-dev - remove side-by-side dev app from /Applications\n"
	@printf "  make app-logs     - stream app logs (predicate: $(LOG_PREDICATE))\n"
	@printf "  make app-logs-all - stream all logs for process Look\n"
	@printf "  (override config) make app-run DEV_CONFIG_PATH=\"$$HOME/.look.dev.config\"\n\n"
	@printf "database\n"
	@printf "  make db-path      - print real db path\n"
	@printf "  make db-status    - inspect candidates/usage tables\n"
	@printf "  make db-shell     - open sqlite shell for real db\n"
	@printf "  make db-reset     - delete real db file\n"
	@printf "  make db-refresh   - delete db and rebuild app\n"

core-check:
	cargo check --workspace --manifest-path core/Cargo.toml

ffi-check:
	cargo check --manifest-path bridge/ffi/Cargo.toml

app-build:
	xcodebuild -project "$(XCODE_PROJECT)" -scheme "$(XCODE_SCHEME)" -configuration "$(XCODE_CONFIG)" -derivedDataPath "$(XCODE_DERIVED_DATA)" build

app-ensure-bundle:
	@if [ ! -d "$(APP_BUNDLE)" ]; then echo "missing app bundle: $(APP_BUNDLE)"; exit 1; fi

app-stop:
	@echo "Stopping any running Look app (including Homebrew install)"
	@osascript -e 'tell application id "$(APP_ID)" to quit' >/dev/null 2>&1 || true
	@pkill -x "$(APP_PROCESS)" >/dev/null 2>&1 || true

app-run: app-build app-stop app-ensure-bundle
	@echo "Opening local app with config: $(DEV_CONFIG_PATH)"
	@$(DEV_OPEN_ENV) open -n "$(APP_BUNDLE)"

app-open: app-build app-ensure-bundle
	@open "$(APP_BUNDLE)"

app-install-dev: app-build app-ensure-bundle
	@echo "Installing side-by-side dev app: $(DEV_APP_INSTALL_BUNDLE)"
	@osascript -e 'tell application id "$(DEV_APP_ID)" to quit' >/dev/null 2>&1 || true
	@rm -rf "$(DEV_APP_INSTALL_BUNDLE)"
	@cp -R "$(APP_BUNDLE)" "$(DEV_APP_INSTALL_BUNDLE)"
	@plutil -replace CFBundleDisplayName -string "$(DEV_APP_NAME)" "$(DEV_APP_INSTALL_BUNDLE)/Contents/Info.plist"
	@plutil -replace CFBundleName -string "$(DEV_APP_NAME)" "$(DEV_APP_INSTALL_BUNDLE)/Contents/Info.plist"
	@plutil -replace CFBundleIdentifier -string "$(DEV_APP_ID)" "$(DEV_APP_INSTALL_BUNDLE)/Contents/Info.plist"
	@codesign --force --sign - --deep "$(DEV_APP_INSTALL_BUNDLE)" >/dev/null
	@"$(LSREGISTER)" -f "$(DEV_APP_INSTALL_BUNDLE)" >/dev/null
	@echo "Installed $(DEV_APP_NAME) with bundle id $(DEV_APP_ID)"

app-run-dev: app-install-dev app-stop
	@echo "Opening side-by-side dev app with config: $(DEV_CONFIG_PATH)"
	@$(DEV_OPEN_ENV) open -a "$(DEV_APP_INSTALL_BUNDLE)"

app-uninstall-dev:
	@echo "Removing side-by-side dev app: $(DEV_APP_INSTALL_BUNDLE)"
	@osascript -e 'tell application id "$(DEV_APP_ID)" to quit' >/dev/null 2>&1 || true
	@rm -rf "$(DEV_APP_INSTALL_BUNDLE)"

app-logs:
	@echo "Streaming logs with predicate: $(LOG_PREDICATE)"
	@log stream --style compact --level debug --predicate '$(LOG_PREDICATE)'

app-logs-all:
	@echo "Streaming all logs for Look process"
	@log stream --style compact --level debug --predicate 'process == "Look"'

symbols: app-build
	@nm -gU "$(APP_DEBUG_DYLIB)" | rg "_look_search_json|_look_record_usage|_look_free_cstring"

db-path:
	@echo "$(REAL_DB_PATH)"

db-status:
	@sqlite3 "$(REAL_DB_PATH)" "SELECT id,title,use_count,last_used_at_unix_s FROM candidates ORDER BY use_count DESC, title ASC LIMIT 20;"
	@echo "---"
	@sqlite3 "$(REAL_DB_PATH)" "SELECT candidate_id,action,used_at_unix_s FROM usage_events ORDER BY id DESC LIMIT 20;"

db-shell:
	sqlite3 "$(REAL_DB_PATH)"

db-reset:
	@rm -f "$(REAL_DB_PATH)"
	@echo "Removed: $(REAL_DB_PATH)"

db-refresh: db-reset app-build
	@echo "DB refreshed and app rebuilt"
