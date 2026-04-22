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
    @StateObject private var appUIState = AppUIState()
    @StateObject private var themeStore = ThemeStore()

    init() {
        if let exitCode = handleCLIFlags() {
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
        let bundleVersion = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
        let bundleBuild = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String
        if bundleVersion != nil || bundleBuild != nil {
            return (bundleVersion, bundleBuild)
        }

        let executablePath = resolvedExecutablePath()
        let executableDir = executablePath.deletingLastPathComponent()

        let directInfoPlist = executableDir
            .deletingLastPathComponent()
            .appendingPathComponent("Info.plist")
        if let info = readInfoPlist(at: directInfoPlist) {
            let version = info["CFBundleShortVersionString"] as? String
            let build = info["CFBundleVersion"] as? String
            if version != nil || build != nil {
                return (version, build)
            }
        }

        var cursor = executablePath.deletingLastPathComponent()
        for _ in 0..<8 {
            if cursor.pathExtension == "app" {
                let infoURL = cursor.appendingPathComponent("Contents/Info.plist")
                if let info = readInfoPlist(at: infoURL) {
                    let version = info["CFBundleShortVersionString"] as? String
                    let build = info["CFBundleVersion"] as? String
                    return (version, build)
                }
                break
            }
            let next = cursor.deletingLastPathComponent()
            if next.path == cursor.path {
                break
            }
            cursor = next
        }

        return (nil, nil)
    }

    private func resolvedExecutablePath() -> URL {
        var size: UInt32 = 0
        _ = _NSGetExecutablePath(nil, &size)
        if size > 0 {
            var buffer = [CChar](repeating: 0, count: Int(size))
            if _NSGetExecutablePath(&buffer, &size) == 0 {
                let path = String(cString: buffer)
                return URL(fileURLWithPath: path).resolvingSymlinksInPath()
            }
        }

        if let firstArg = CommandLine.arguments.first,
            firstArg.hasPrefix("/")
        {
            return URL(fileURLWithPath: firstArg).resolvingSymlinksInPath()
        }

        return URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent(CommandLine.arguments.first ?? "Look")
            .resolvingSymlinksInPath()
    }

    private func readInfoPlist(at url: URL) -> [String: Any]? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        guard
            let plist = try? PropertyListSerialization.propertyList(
                from: data,
                options: [],
                format: nil
            )
        else {
            return nil
        }
        return plist as? [String: Any]
    }

    var body: some Scene {
        WindowGroup(id: "main") {
            ContentView()
                .frame(minWidth: 620, minHeight: 600)
                .background(WindowConfigurator())
                .environmentObject(appUIState)
                .environmentObject(themeStore)
        }
        .windowStyle(.hiddenTitleBar)
        .commands {
            CommandGroup(replacing: .newItem) {}

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
                        appUIState.showsThemeSettings.toggle()
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
