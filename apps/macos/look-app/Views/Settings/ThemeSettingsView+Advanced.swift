import AppKit
import SwiftUI
import UniformTypeIdentifiers

extension ThemeSettingsView {
    var backgroundTab: some View {
        VStack(alignment: .leading, spacing: 10) {
            ScrollViewReader { proxy in
                ScrollView(.vertical, showsIndicators: false) {
                    VStack(alignment: .leading, spacing: 10) {
                        AISettingsSection(settings: $settings)

                        Divider()
                            .overlay(themeStore.dividerColor())
                            .padding(.vertical, 4)

                        Text("Web search")
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                            .foregroundStyle(themeStore.secondaryTextColor())

                        settingRow(
                            "Search Engine",
                            hint: "google, duckduckgo, bing, or a https:// URL with {encoded}",
                            alignment: .top
                        ) {
                            VStack(alignment: .leading, spacing: 4) {
                                TextField(ThemeSettings.default.webSearchEngine, text: $webSearchEngineInput)
                                    .textFieldStyle(.roundedBorder)
                                    .foregroundStyle(webSearchEngineRefused ? themeStore.dangerColor() : Color.primary)
                                    .overlay {
                                        if webSearchEngineRefused {
                                            RoundedRectangle(cornerRadius: 5)
                                                .stroke(themeStore.dangerColor(), lineWidth: 1)
                                        }
                                    }
                                    .onSubmit { applyWebSearchEngineInput() }
                                Text("e.g. https://search.yahoo.com/search?p={encoded}")
                                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                    .foregroundStyle(themeStore.mutedTextColor())
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                        }

                        Divider()
                            .overlay(themeStore.dividerColor())
                            .padding(.vertical, 4)

                        Text("Background")
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                            .foregroundStyle(themeStore.secondaryTextColor())

                        settingRow("Image") {
                            Button("Choose Image") { selectBackgroundImage() }
                        } trailing: {
                            Text(settings.backgroundImagePath ?? "No image selected")
                                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                .foregroundStyle(themeStore.mutedTextColor())
                                .lineLimit(1)
                                .truncationMode(.middle)

                            if settings.backgroundImagePath != nil {
                                Button("Clear") {
                                    withAnimation(Motion.Fade.animation) {
                                        themeStore.setBackgroundImage(url: nil)
                                    }
                                }
                            }
                        }

                        settingRow("Image Layout", hint: settings.backgroundImageMode.detail) {
                            Picker("Image Layout", selection: $settings.backgroundImageMode) {
                                ForEach(BackgroundImageMode.allCases) { mode in
                                    Text(mode.title).tag(mode)
                                }
                            }
                            .pickerStyle(.menu)
                            .labelsHidden()
                        }

                        LabeledSlider(title: "Image Opacity", value: $settings.backgroundImageOpacity, range: 0...1)
                        LabeledSlider(title: "Image Blur", value: $settings.backgroundImageBlur, range: 0...30)

                        Divider()
                            .overlay(themeStore.dividerColor())
                            .padding(.vertical, 4)

                        Text("Indexing")
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                            .foregroundStyle(themeStore.secondaryTextColor())

                        settingRow("File Scan Depth", hint: "How many directory levels to index") {
                            TextField("4", text: $fileScanDepthInput)
                                .textFieldStyle(.roundedBorder)
                                .onChange(of: fileScanDepthInput) { _, value in
                                    fileScanDepthInput = sanitizedNumericInput(value)
                                    if let parsed = Int(fileScanDepthInput) {
                                        if parsed >= AppConstants.FileScan.minDepth
                                            && parsed <= AppConstants.FileScan.maxDepth
                                        {
                                            settings.fileScanDepth = parsed
                                            fileScanDepthError = nil
                                        } else {
                                            fileScanDepthError =
                                                "Must be \(AppConstants.FileScan.minDepth)-\(AppConstants.FileScan.maxDepth)"
                                        }
                                    }
                                }
                                .help("Valid: \(AppConstants.FileScan.minDepth)-\(AppConstants.FileScan.maxDepth)")
                        } trailing: {
                            if let error = fileScanDepthError {
                                Text(error)
                                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                    .foregroundStyle(themeStore.dangerColor())
                            }
                        }

                        settingRow("File Scan Limit", hint: "Max files indexed per refresh") {
                            TextField(String(AppConstants.FileScan.defaultLimit), text: $fileScanLimitInput)
                                .textFieldStyle(.roundedBorder)
                                .onChange(of: fileScanLimitInput) { _, value in
                                    fileScanLimitInput = sanitizedNumericInput(value)
                                    if let parsed = Int(fileScanLimitInput) {
                                        if parsed >= AppConstants.FileScan.minLimit
                                            && parsed <= AppConstants.FileScan.maxLimit
                                        {
                                            settings.fileScanLimit = parsed
                                            fileScanLimitError = nil
                                        } else {
                                            fileScanLimitError =
                                                "Must be \(AppConstants.FileScan.minLimit)-\(AppConstants.FileScan.maxLimit)"
                                        }
                                    }
                                }
                        } trailing: {
                            if let error = fileScanLimitError {
                                Text(error)
                                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                    .foregroundStyle(themeStore.dangerColor())
                            }
                        }

                        settingRow(
                            "Lazy indexing",
                            hint: "Refresh index automatically when launcher opens after file/app changes"
                        ) {
                            Toggle("", isOn: $settings.lazyIndexingEnabled).labelsHidden()
                        }

                        settingRow("Extra Scan Dirs", alignment: .top) {
                            Button("Add Directory") {
                                selectExtraScanDirectory()
                            }
                        } trailing: {
                            VStack(alignment: .leading, spacing: 8) {
                                if themeStore.extraFileScanRoots.isEmpty {
                                    Text("No extra scan directories")
                                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                        .foregroundStyle(themeStore.mutedTextColor())
                                } else {
                                    ScrollView(.horizontal) {
                                        HStack(spacing: 8) {
                                            ForEach(themeStore.extraFileScanRoots, id: \.self) { path in
                                                RemovablePill(themeStore: themeStore, removeLabel: "Remove \(path)") {
                                                    themeStore.removeExtraFileScanRoot(path)
                                                    extraScanDirectoryMessage = nil
                                                } label: {
                                                    Text(path).lineLimit(1)
                                                }
                                            }
                                        }
                                    }
                                    .scrollIndicators(.hidden)
                                }

                                if let extraScanDirectoryMessage {
                                    Text(extraScanDirectoryMessage)
                                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                        .foregroundStyle(themeStore.dangerColor())
                                }

                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }

                        settingRow("Skip Folders", alignment: .top) {
                            Button("Add Folder") {
                                selectExcludedFolderPath()
                            }
                        } trailing: {
                            VStack(alignment: .leading, spacing: 8) {
                                if themeStore.excludedFolderPaths.isEmpty {
                                    Text("No excluded folder paths yet")
                                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                        .foregroundStyle(themeStore.mutedTextColor())
                                } else {
                                    ScrollView(.horizontal) {
                                        HStack(spacing: 8) {
                                            ForEach(themeStore.excludedFolderPaths, id: \.self) { path in
                                                RemovablePill(themeStore: themeStore, removeLabel: "Remove \(path)") {
                                                    themeStore.removeExcludedFolderPath(path)
                                                } label: {
                                                    Text(path).lineLimit(1)
                                                }
                                            }
                                        }
                                    }
                                    .scrollIndicators(.hidden)
                                }

                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }

                        Divider()
                            .overlay(themeStore.dividerColor())
                            .padding(.vertical, 4)

                        Text("Privacy & Logs")
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                            .foregroundStyle(themeStore.secondaryTextColor())

                        settingRow(
                            "Backend Log Level",
                            hint: "Error only by default; use Info/Debug for troubleshooting"
                        ) {
                            Picker("Backend Log Level", selection: $settings.backendLogLevel) {
                                ForEach(BackendLogLevel.allCases) { level in
                                    Text(level.title).tag(level)
                                }
                            }
                            .pickerStyle(.menu)
                            .labelsHidden()
                        }

                        Divider()
                            .overlay(themeStore.dividerColor())
                            .padding(.vertical, 4)

                        Text("Startup")
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                            .foregroundStyle(themeStore.secondaryTextColor())

                        settingRow("Launch at login", hint: "Start look automatically when you sign in") {
                            Toggle("", isOn: $settings.launchAtLogin).labelsHidden()
                        }

                        Divider()
                            .overlay(themeStore.dividerColor())
                            .padding(.vertical, 4)

                        Text("Config file")
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                            .foregroundStyle(themeStore.secondaryTextColor())

                        settingRow(
                            nil,
                            hint: "Regenerate a fresh default config file. Your current file will be replaced."
                        ) {
                            Button("Create Fresh Config") {
                                showFreshConfigConfirm = true
                                freshConfigMessage = nil
                            }
                        } trailing: {
                            if let freshConfigMessage {
                                Text(freshConfigMessage)
                                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                    .foregroundStyle(themeStore.mutedTextColor())
                            }
                        }

                        Divider()
                            .overlay(themeStore.dividerColor())
                            .padding(.vertical, 4)

                        aboutSection
                            .id(Self.aboutAnchorID)

                    }
                }
                .onAppear {
                    syncIndexingInputsFromSettings()
                    webSearchEngineInput = settings.webSearchEngine
                    webSearchEngineLoaded = true
                }
                .onChange(of: settings.fileScanDepth) { _, _ in
                    fileScanDepthInput = String(settings.fileScanDepth)
                }
                .onChange(of: settings.fileScanLimit) { _, _ in
                    fileScanLimitInput = String(settings.fileScanLimit)
                }
                // Reveal the About/update result after a manual "Check for Updates".
                .onChange(of: updateChecker.statusMessage) { _, message in
                    guard message != nil else { return }
                    withAnimation {
                        proxy.scrollTo(Self.aboutAnchorID, anchor: .bottom)
                    }
                }
            }
        }
    }

    static let aboutAnchorID = "look-about-section"

    /// One view: `.id` on a builder pair tags both, and the scroll landed on
    /// the heading.
    var aboutSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("About")
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                .foregroundStyle(themeStore.secondaryTextColor())

            AppUpdateStatusView(themeStore: themeStore)
        }
    }

    func selectBackgroundImage() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.allowedContentTypes = [.image]
        if FilePanel.runModal(panel) == .OK {
            withAnimation(Motion.Fade.animation) {
                themeStore.setBackgroundImage(url: panel.url)
            }
        }
    }

    func selectExcludedFolderPath() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        if FilePanel.runModal(panel) == .OK, let url = panel.url {
            withAnimation(Motion.Insert.animation) {
                themeStore.addExcludedFolderPath(url: url)
            }
        }
    }

    func selectExtraScanDirectory() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        if FilePanel.runModal(panel) == .OK, let url = panel.url {
            withAnimation(Motion.Insert.animation) {
                if let error = themeStore.addExtraFileScanRoot(url: url) {
                    extraScanDirectoryMessage = error.message
                } else {
                    extraScanDirectoryMessage = nil
                }
            }
        }
    }

    func syncIndexingInputsFromSettings() {
        fileScanDepthInput = String(settings.fileScanDepth)
        fileScanLimitInput = String(settings.fileScanLimit)
    }

    /// Whether the core refuses what is in the box: it is drawn red, and the
    /// user fixes it when they like. Searches use Google meanwhile.
    var webSearchEngineRefused: Bool {
        guard webSearchEngineLoaded else { return false }
        let trimmed = webSearchEngineInput.trimmingCharacters(in: .whitespacesAndNewlines)
        return !EngineBridge.shared.isValidWebSearch(trimmed)
    }

    /// Keeps what was typed, a refused value included: Save writes it as is.
    func applyWebSearchEngineInput() {
        // The box is filled when the Advanced tab appears. Before that, Save
        // must not read its empty text as the default over a saved engine.
        guard webSearchEngineLoaded else { return }
        let trimmed = webSearchEngineInput.trimmingCharacters(in: .whitespacesAndNewlines)
        let value = trimmed.isEmpty ? ThemeSettings.default.webSearchEngine : trimmed
        webSearchEngineInput = value
        settings.webSearchEngine = value
    }

    func sanitizedNumericInput(_ value: String) -> String {
        String(value.filter(\.isNumber))
    }

    func applyFileScanDepthInput() {
        guard let parsed = Int(fileScanDepthInput), parsed > 0 else {
            fileScanDepthInput = String(settings.fileScanDepth)
            return
        }
        settings.fileScanDepth = min(max(1, parsed), 12)
        fileScanDepthInput = String(settings.fileScanDepth)
    }

    func applyFileScanLimitInput() {
        guard let parsed = Int(fileScanLimitInput), parsed > 0 else {
            fileScanLimitInput = String(settings.fileScanLimit)
            return
        }
        settings.fileScanLimit = min(max(500, parsed), 50_000)
        fileScanLimitInput = String(settings.fileScanLimit)
    }

    func runFreshConfigReset() {
        let ok = themeStore.regenerateFreshConfigFile()
        syncIndexingInputsFromSettings()
        freshConfigMessage = ok ? "Fresh config created" : "Failed to recreate config"
        if ok {
            NotificationCenter.default.post(name: .lookReloadConfigRequested, object: nil)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
            freshConfigMessage = nil
        }
        NotificationCenter.default.post(name: .lookFocusSettingsInputRequested, object: nil)
    }

    var hasIndexingError: Bool {
        fileScanDepthError != nil || fileScanLimitError != nil
    }
}
