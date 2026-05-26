// swift-tools-version: 5.10
import PackageDescription

let package = Package(
    name: "LauncherLogic",
    platforms: [
        .macOS(.v13),
    ],
    products: [
        .library(name: "LauncherLogic", targets: ["LauncherLogic"]),
    ],
    targets: [
        .target(
            name: "LauncherLogic",
            path: "look-app",
            sources: [
                "Support/Launcher/HintText.swift",
                "Support/AppConstants.swift",
                "Support/Launcher/LauncherSearchLogic.swift",
                "Support/Launcher/BridgeErrorMapping.swift",
                "Support/SingleInstanceLock.swift",
                "Models/LauncherResult.swift",
                "Views/CalcCommand.swift",
            ]
        ),
        .testTarget(
            name: "LauncherLogicTests",
            dependencies: ["LauncherLogic"],
            path: "LauncherLogicTests"
        ),
    ]
)
