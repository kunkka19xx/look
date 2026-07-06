import AppKit
import SwiftUI
import UniformTypeIdentifiers

extension ThemeSettingsView {
    var aiAvailability: AIProviderAvailability {
        AIQueryRouter.shared.availability(of: settings.aiProvider)
    }

    @ViewBuilder
    var aiInfoIndicator: some View {
        Image(systemName: aiAvailability.isAvailable ? "checkmark.circle.fill" : "info.circle")
            .font(.system(size: CGFloat(settings.fontSize)))
            .foregroundStyle(
                aiAvailability.isAvailable
                    ? Color.green.opacity(0.85)
                    : themeStore.secondaryTextColor()
            )
            .contentShape(Rectangle())
            .accessibilityLabel(Text("AI availability"))
            .hoverTooltip(aiAvailabilityTooltip)
    }

    var aiAvailabilityTooltip: String {
        let base = "Shows instant answers (facts, definitions, weather, currency, "
            + "crypto, calculations) and web search suggestions for your queries. "
            + "Powered by free web sources (Wikipedia, DuckDuckGo) plus on-device "
            + "Apple Intelligence where available. Queries are sent to the web "
            + "while this is on."
        switch aiAvailability {
        case .available:
            return base + "\n\nApple Intelligence: Ready on this Mac."
        case .unavailable(let reason):
            return base + "\n\nApple Intelligence: \(reason.userFacingMessage) "
                + "(Web answers and suggestions still work without it.)"
        }
    }

    var backgroundTab: some View {
        VStack(alignment: .leading, spacing: 10) {
            ScrollViewReader { proxy in
            ScrollView(.vertical, showsIndicators: false) {
                VStack(alignment: .leading, spacing: 10) {
                    Text("AI")
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                        .foregroundStyle(themeStore.secondaryTextColor())

                    HStack(spacing: 10) {
                        Text("AI & web answers")
                            .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                            .foregroundStyle(themeStore.secondaryTextColor())

                        // Not gated on Apple Intelligence availability - the web
                        // answer card and search suggestions work without it; the
                        // on-device tier just self-skips when unavailable.
                        Toggle("Enable AI & web answers", isOn: $settings.aiEnabled)
                            .toggleStyle(.switch)
                            .labelsHidden()
                            .help("Show instant answers and web search suggestions (sends queries to the web; opt-out anytime)")

                        aiInfoIndicator

                        Spacer(minLength: 0)
                    }

                    Divider()
                        .overlay(.white.opacity(0.1))
                        .padding(.vertical, 4)

                    Text("Background")
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                        .foregroundStyle(themeStore.secondaryTextColor())

                    HStack {
                        Button("Choose Background Image") {
                            selectBackgroundImage()
                        }
                        if settings.backgroundImagePath != nil {
                            Button("Clear") {
                                themeStore.setBackgroundImage(url: nil)
                            }
                        }
                    }

                    Text(settings.backgroundImagePath ?? "No image selected")
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.secondaryTextColor())
                        .lineLimit(1)

                    HStack(spacing: 10) {
                        Text("Image Layout")
                            .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                            .foregroundStyle(themeStore.secondaryTextColor())

                        Picker("Image Layout", selection: $settings.backgroundImageMode) {
                            ForEach(BackgroundImageMode.allCases) { mode in
                                Text(mode.title).tag(mode)
                            }
                        }
                        .pickerStyle(.menu)
                        .labelsHidden()
                        .frame(width: AppConstants.ThemeUI.pickerWidth)

                        Text(settings.backgroundImageMode.detail)
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                            .foregroundStyle(themeStore.mutedTextColor())
                            .lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }

                    LabeledSlider(title: "Image Opacity", value: $settings.backgroundImageOpacity, range: 0...1)
                    LabeledSlider(title: "Image Blur", value: $settings.backgroundImageBlur, range: 0...30)

                    Divider()
                        .overlay(.white.opacity(0.1))
                        .padding(.vertical, 4)

                    Text("Indexing")
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                        .foregroundStyle(themeStore.secondaryTextColor())

                    HStack(spacing: 10) {
                        Text("File Scan Depth")
                            .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                            .foregroundStyle(themeStore.secondaryTextColor())

                        TextField("4", text: $fileScanDepthInput)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 80, alignment: .leading)
                            .onChange(of: fileScanDepthInput) { _, value in
                                fileScanDepthInput = sanitizedNumericInput(value)
                                if let parsed = Int(fileScanDepthInput) {
                                    if parsed >= AppConstants.FileScan.minDepth && parsed <= AppConstants.FileScan.maxDepth {
                                        settings.fileScanDepth = parsed
                                        fileScanDepthError = nil
                                    } else {
                                        fileScanDepthError = "Must be \(AppConstants.FileScan.minDepth)-\(AppConstants.FileScan.maxDepth)"
                                    }
                                }
                            }
                            .help("Valid: \(AppConstants.FileScan.minDepth)-\(AppConstants.FileScan.maxDepth)")

                        if let error = fileScanDepthError {
                            Text(error)
                                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                .foregroundStyle(themeStore.dangerColor())
                        }

                        Text("How many directory levels to index")
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                            .foregroundStyle(themeStore.mutedTextColor())
                            .lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }

                    HStack(spacing: 10) {
                        Text("File Scan Limit")
                            .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                            .foregroundStyle(themeStore.secondaryTextColor())

                        TextField("4000", text: $fileScanLimitInput)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 100, alignment: .leading)
                            .onChange(of: fileScanLimitInput) { _, value in
                                fileScanLimitInput = sanitizedNumericInput(value)
                                if let parsed = Int(fileScanLimitInput) {
                                    if parsed >= AppConstants.FileScan.minLimit && parsed <= AppConstants.FileScan.maxLimit {
                                        settings.fileScanLimit = parsed
                                        fileScanLimitError = nil
                                    } else {
                                        fileScanLimitError = "Must be \(AppConstants.FileScan.minLimit)-\(AppConstants.FileScan.maxLimit)"
                                    }
                                }
                            }

                        if let error = fileScanLimitError {
                            Text(error)
                                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                .foregroundStyle(themeStore.dangerColor())
                        }

                        Text("Max files indexed per refresh")
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                            .foregroundStyle(themeStore.mutedTextColor())
                            .lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }

                    Toggle(isOn: $settings.lazyIndexingEnabled) {
                        HStack(spacing: 10) {
                            Text("Lazy indexing")
                                .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                            Text("Refresh index automatically when launcher opens after file/app changes")
                                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                .foregroundStyle(themeStore.mutedTextColor())
                                .lineLimit(1)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }

                    HStack(alignment: .top, spacing: 10) {
                        Text("Extra Scan Dirs")
                            .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                            .foregroundStyle(themeStore.secondaryTextColor())

                        VStack(alignment: .leading, spacing: 8) {
                            Button("Add Directory") {
                                selectExtraScanDirectory()
                            }

                            if themeStore.extraFileScanRoots.isEmpty {
                                Text("No extra scan directories")
                                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                    .foregroundStyle(themeStore.mutedTextColor())
                            } else {
                                ScrollView(.horizontal) {
                                    HStack(spacing: 8) {
                                        ForEach(themeStore.extraFileScanRoots, id: \.self) { path in
                                            HStack(spacing: 6) {
                                                Text(path)
                                                    .lineLimit(1)
                                                Button {
                                                    themeStore.removeExtraFileScanRoot(path)
                                                    extraScanDirectoryMessage = nil
                                                } label: {
                                                    Image(systemName: "xmark")
                                                        .font(.system(size: 10, weight: .semibold))
                                                }
                                                .buttonStyle(.plain)
                                            }
                                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                            .foregroundStyle(themeStore.secondaryTextColor())
                                            .padding(.horizontal, 9)
                                            .padding(.vertical, 5)
                                            .background(.white.opacity(0.12), in: Capsule())
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

                    HStack(alignment: .top, spacing: 10) {
                        Text("Skip Folders")
                            .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                            .foregroundStyle(themeStore.secondaryTextColor())

                        VStack(alignment: .leading, spacing: 8) {
                            Button("Add Folder") {
                                selectExcludedFolderPath()
                            }

                            if themeStore.excludedFolderPaths.isEmpty {
                                Text("No excluded folder paths yet")
                                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                    .foregroundStyle(themeStore.mutedTextColor())
                            } else {
                                ScrollView(.horizontal) {
                                    HStack(spacing: 8) {
                                        ForEach(themeStore.excludedFolderPaths, id: \.self) { path in
                                            HStack(spacing: 6) {
                                                Text(path)
                                                    .lineLimit(1)
                                                Button {
                                                    themeStore.removeExcludedFolderPath(path)
                                                } label: {
                                                    Image(systemName: "xmark")
                                                        .font(.system(size: 10, weight: .semibold))
                                                }
                                                .buttonStyle(.plain)
                                            }
                                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                            .foregroundStyle(themeStore.secondaryTextColor())
                                            .padding(.horizontal, 9)
                                            .padding(.vertical, 5)
                                            .background(.white.opacity(0.12), in: Capsule())
                                        }
                                    }
                                }
                                .scrollIndicators(.hidden)
                            }

                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }

                    Divider()
                        .overlay(.white.opacity(0.1))
                        .padding(.vertical, 4)

                    Text("Privacy & Logs")
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                        .foregroundStyle(themeStore.secondaryTextColor())

                    HStack(spacing: 10) {
                        Text("Backend Log Level")
                            .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                            .foregroundStyle(themeStore.secondaryTextColor())

                        Picker("Backend Log Level", selection: $settings.backendLogLevel) {
                            ForEach(BackendLogLevel.allCases) { level in
                                Text(level.title).tag(level)
                            }
                        }
                        .pickerStyle(.menu)
                        .labelsHidden()
                        .frame(width: AppConstants.ThemeUI.pickerWidth)

                        Text("Error only by default; use Info/Debug for troubleshooting")
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                            .foregroundStyle(themeStore.mutedTextColor())
                            .lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }

                    Divider()
                        .overlay(.white.opacity(0.1))
                        .padding(.vertical, 4)

                    Text("Startup")
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                        .foregroundStyle(themeStore.secondaryTextColor())

                    Toggle(isOn: $settings.launchAtLogin) {
                        HStack(spacing: 10) {
                            Text("Launch at login")
                                .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                            Text("Start look automatically when you sign in")
                                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                .foregroundStyle(themeStore.mutedTextColor())
                                .lineLimit(1)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }

                    Divider()
                        .overlay(.white.opacity(0.1))
                        .padding(.vertical, 4)

                    Text("Config file")
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                        .foregroundStyle(themeStore.secondaryTextColor())

                    HStack(spacing: 10) {
                        Button("Create Fresh Config") {
                            showFreshConfigConfirm = true
                            freshConfigMessage = nil
                        }

                        Text("Regenerate a fresh default config file. Your current file will be replaced.")
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                            .foregroundStyle(themeStore.mutedTextColor())
                            .lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)

                        if let freshConfigMessage {
                            Text(freshConfigMessage)
                                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                                .foregroundStyle(themeStore.mutedTextColor())
                        }
                    }

                    Divider()
                        .overlay(.white.opacity(0.1))
                        .padding(.vertical, 4)

                    aboutSection
                        .id(Self.aboutAnchorID)

                }
            }
            .onAppear { syncIndexingInputsFromSettings() }
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

            Spacer(minLength: 0)

            Text(HintText.Settings.advancedApply)
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())
        }
    }

    static let aboutAnchorID = "look-about-section"

    @ViewBuilder
    var aboutSection: some View {
        Text("About")
            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
            .foregroundStyle(themeStore.secondaryTextColor())

        AppUpdateStatusView(themeStore: themeStore)
    }

    func selectBackgroundImage() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.allowedContentTypes = [.image]
        if panel.runModal() == .OK {
            themeStore.setBackgroundImage(url: panel.url)
        }
    }

    func selectExcludedFolderPath() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        if panel.runModal() == .OK, let url = panel.url {
            themeStore.addExcludedFolderPath(url: url)
        }
    }

    func selectExtraScanDirectory() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        if panel.runModal() == .OK, let url = panel.url {
            if let error = themeStore.addExtraFileScanRoot(url: url) {
                extraScanDirectoryMessage = error.message
            } else {
                extraScanDirectoryMessage = nil
            }
        }
    }

    func syncIndexingInputsFromSettings() {
        fileScanDepthInput = String(settings.fileScanDepth)
        fileScanLimitInput = String(settings.fileScanLimit)
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
