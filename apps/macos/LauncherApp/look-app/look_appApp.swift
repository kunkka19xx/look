//
//  look_appApp.swift
//  look-app
//
//  Created by kunkka07xx on 2026/04/04.
//

import Darwin
import Foundation
import SwiftUI

@main
struct look_appApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    // The launcher window is owned by AppDelegate (an AppKit NSWindow), not a
    // SwiftUI WindowGroup - see AppDelegate.makeLauncherWindow() for why. This
    // store is shared with that window's hosted ContentView.
    private let themeStore = ThemeStore.shared

    init() {
        if let exitCode = handleCLIFlags() {
            fflush(stdout)
            exit(exitCode)
        }

        if let exitCode = LaunchModes.handleLaunchArguments() {
            fflush(stdout)
            exit(exitCode)
        }

        ConfigPathResolver.applyDefaultConfigEnvironmentIfNeeded()
    }

    private func handleCLIFlags() -> Int32? {
        if CommandLine.arguments.contains("-v") || CommandLine.arguments.contains("--version") {
            let versionInfo = readVersionInfo()
            let version = versionInfo.version
            let build = versionInfo.build
            if let version {
                if let build, build != version {
                    print("look \(version) (\(build))")
                } else {
                    print("look \(version)")
                }
            } else {
                print("look unknown")
            }
            return 0
        }

        return nil
    }

    private func readVersionInfo() -> (version: String?, build: String?) {
        let bundle = AppBundle.bundle
        return (
            bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String,
            bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String
        )
    }

    var body: some Scene {
        // The launcher window is an AppKit NSWindow owned by AppDelegate (see
        // AppDelegate.makeLauncherWindow) - SwiftUI won't create a WindowGroup
        // window on a background login launch, which was the root cause of the
        // dead Cmd+Space. A Settings scene gives the app a valid Scene to carry
        // the command menu below without auto-creating any window.
        Settings {
            EmptyView()
        }
        .commands {
            CommandGroup(replacing: .newItem) {}

            // The Settings scene above only keeps SwiftUI's command
            // infrastructure alive. Remove macOS's default Cmd+, action so the
            // documented Cmd+Shift+, shortcut is the only way to open settings.
            CommandGroup(replacing: .appSettings) {}

            CommandGroup(replacing: .appTermination) {
                Button("Hide Look") {
                    NotificationCenter.default.post(name: .lookHideLauncherRequested, object: nil)
                }
                .keyboardShortcut("q", modifiers: [.command])

                Button("Quit Look") {
                    NSApplication.shared.terminate(nil)
                }
                .keyboardShortcut("q", modifiers: [.command, .option])
            }

            CommandGroup(after: .appSettings) {
                Button("Theme Settings") {
                    DispatchQueue.main.async {
                        NotificationCenter.default.post(name: .lookToggleSettingsRequested, object: nil)
                    }
                }
                .keyboardShortcut(",", modifiers: [.command, .shift])

                Button("Reload Config") {
                    DispatchQueue.main.async {
                        NotificationCenter.default.post(name: .lookReloadConfigRequested, object: nil)
                    }
                }
                .keyboardShortcut(";", modifiers: [.command, .shift])

                Divider()

                Button("Zoom In") {
                    DispatchQueue.main.async {
                        themeStore.zoomIn()
                    }
                }
                .keyboardShortcut("=", modifiers: [.command])

                Button("Zoom Out") {
                    DispatchQueue.main.async {
                        themeStore.zoomOut()
                    }
                }
                .keyboardShortcut("-", modifiers: [.command])

                Button("Actual Size") {
                    DispatchQueue.main.async {
                        themeStore.resetZoom()
                    }
                }
                .keyboardShortcut("0", modifiers: [.command])
            }
        }
    }
}
