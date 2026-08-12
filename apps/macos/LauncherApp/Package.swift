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
                "Support/ConfigFileLines.swift",
                "Support/Launcher/LauncherSearchLogic.swift",
                "Support/Launcher/ProcessScoring.swift",
                "Support/Launcher/DeleteTargetLogic.swift",
                "Support/Launcher/BridgeErrorMapping.swift",
                "Support/Launcher/SyntheticRow.swift",
                "Support/AI/OllamaCodec.swift",
                "Support/AI/LocalHostCheck.swift",
                "Support/Actions/ActionTypes.swift",
                "Support/Actions/DatePhrase.swift",
                "Support/Actions/ActionResolution.swift",
                "Support/Actions/ScheduleWords.swift",
                "Support/SingleInstanceLock.swift",
                "Models/LauncherResult.swift",
            ]
        ),
        .testTarget(
            name: "LauncherLogicTests",
            dependencies: ["LauncherLogic"],
            path: "LauncherLogicTests"
        ),
    ]
)
