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
                "Support/ShortcutCatalog.swift",
                "Support/ConfigFileLines.swift",
                "Support/Launcher/LauncherSearchLogic.swift",
                "Support/Launcher/ProcessScoring.swift",
                "Support/Launcher/DeleteTargetLogic.swift",
                "Support/Launcher/RevealTargetLogic.swift",
                "Support/Launcher/BridgeErrorMapping.swift",
                "Support/Launcher/SyntheticRow.swift",
                "Support/Launcher/PreviewText.swift",
                "Support/AI/OllamaCodec.swift",
                "Support/AI/AIRequest.swift",
                "Support/AI/LocalHostCheck.swift",
                "Support/Actions/ActionTypes.swift",
                "Support/Actions/DatePhrase.swift",
                "Support/Actions/ActionResolution.swift",
                "Support/Actions/ScheduleWords.swift",
                "Support/Actions/TextOpSource.swift",
                "Support/Actions/ExtractedTextQuality.swift",
                "Support/Actions/TurnLedger.swift",
                "Support/Actions/MentionQuery.swift",
                "Support/Actions/MentionAttachments.swift",
                "Support/SingleInstanceLock.swift",
                "Models/LauncherResult.swift",
                "Models/SourceLevel.swift",
                "Support/Launcher/SourceLevelStack.swift",
            ]
        ),
        .testTarget(
            name: "LauncherLogicTests",
            dependencies: ["LauncherLogic"],
            path: "LauncherLogicTests"
        ),
    ]
)
