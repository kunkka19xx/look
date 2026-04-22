import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct ThemeSettingsView: View {
    private enum Field {
        case fontName
    }

    @EnvironmentObject private var appUIState: AppUIState
    @EnvironmentObject private var themeStore: ThemeStore
    @Binding var settings: ThemeSettings
    @State private var selectedTab = 0
    @State private var saveMessage: String?
    @State private var fontSuggestions: [String] = []
    @State private var showsFontSuggestions = false
    @State private var isPickingFontSuggestion = false
    @State private var fileScanDepthInput = ""
    @State private var fileScanLimitInput = ""
    @State private var fileScanDepthError: String?
    @State private var fileScanLimitError: String?
    @State private var extraScanDirectoryMessage: String?
    @State private var showFreshConfigConfirm = false
    @State private var freshConfigMessage: String?
    @State private var localKeyMonitor: Any?
    @FocusState private var focusedField: Field?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Settings")
                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize + 2), weight: .semibold))
                Spacer()

                if let saveMessage {
                    Text(saveMessage)
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                        .foregroundStyle(.white)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 5)
                        .background(.green.opacity(0.42), in: Capsule())
                }

                Button("Save Config") {
                    applyFileScanDepthInput()
                    applyFileScanLimitInput()
                    let ok = themeStore.saveCurrentConfigToFile()
                    saveMessage = ok ? "Saved" : "Save failed"
                    if ok {
                        NotificationCenter.default.post(name: .lookReloadConfigRequested, object: nil)
                    }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.6) {
                        saveMessage = nil
                    }
                    NotificationCenter.default.post(name: .lookFocusSettingsInputRequested, object: nil)
                }
                .disabled(hasIndexingError)
                .opacity(hasIndexingError ? 0.5 : 1)
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))

                Button("Back to Launcher") {
                    closeSettingsPanel()
                }
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                Text("Esc or Cmd+Shift+, to close")
                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
            }

            HStack(spacing: 8) {
                tabButton(title: "Appearance", index: 0)
                tabButton(title: "Advanced", index: 1)
                tabButton(title: "Shortcuts", index: 2)
            }

            Group {
                if selectedTab == 0 {
                    appearanceTab
                } else if selectedTab == 1 {
                    backgroundTab
                } else {
                    shortcutsTab
                }
            }
            .frame(maxHeight: .infinity, alignment: .top)

        }
        .onExitCommand {
            closeSettingsPanel()
        }
        .onAppear {
            installLocalKeyMonitorIfNeeded()
        }
        .onDisappear {
            removeLocalKeyMonitor()
        }
        .alert("Create fresh config file?", isPresented: $showFreshConfigConfirm) {
            Button("Cancel", role: .cancel) {}
            Button("Create Fresh Config", role: .destructive) {
                runFreshConfigReset()
            }
        } message: {
            Text("This will replace your current config file with default values.")
        }
    }

    private var appearanceTab: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 10) {
                sectionHeaderWithPicker("Theme") {
                    Picker("Theme", selection: $settings.uiTheme) {
                        ForEach(BuiltinThemePreset.allCases) { preset in
                            Text(preset.title).tag(preset)
                        }
                    }
                    .pickerStyle(.menu)
                    .labelsHidden()
                    .frame(width: AppConstants.ThemeUI.pickerWidth)
                    .onChange(of: settings.uiTheme) { _, newValue in
                        themeStore.applyBuiltinTheme(newValue)
                    }
                }

                Divider()
                    .overlay(.white.opacity(0.1))
                    .padding(.vertical, 4)

                sectionHeader("Tint Color")

                LabeledSlider(title: "Red", value: $settings.tintRed, range: 0...1)
                LabeledSlider(title: "Green", value: $settings.tintGreen, range: 0...1)
                LabeledSlider(title: "Blue", value: $settings.tintBlue, range: 0...1)
                LabeledSlider(title: "Tint Opacity", value: $settings.tintOpacity, range: 0...1)

                sectionHeader("Blur")

                LabeledSlider(title: "Blur Opacity", value: $settings.blurOpacity, range: 0...1)
                LabeledSlider(title: "Settings Blur", value: $appUIState.settingsBlurMultiplier, range: 0.4...1)

                HStack(spacing: 10) {
                    Text("Blur Style")
                        .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.secondaryTextColor())

                    Picker("Blur Style", selection: $settings.blurMaterial) {
                        ForEach(LauncherBlurMaterial.allCases) { item in
                            Text(item.title).tag(item)
                        }
                    }
                    .pickerStyle(.menu)
                    .labelsHidden()
                    .frame(width: AppConstants.ThemeUI.pickerWidth)

                    Text(settings.blurMaterial.detail)
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                        .lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }

                sectionHeader("Font")

                HStack(spacing: 10) {
                    Text("Font Name")
                        .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.secondaryTextColor())

                    TextField("SF Pro Text", text: $settings.fontName)
                        .textFieldStyle(.roundedBorder)
                        .focused($focusedField, equals: .fontName)
                        .onTapGesture {
                            focusedField = .fontName
                            fontSuggestions = themeStore.fontNameSuggestions(for: settings.fontName, limit: 24)
                            showsFontSuggestions = true
                        }
                        .onChange(of: settings.fontName) { _, newValue in
                            if isPickingFontSuggestion {
                                return
                            }
                            fontSuggestions = themeStore.fontNameSuggestions(for: newValue, limit: 24)
                            showsFontSuggestions = focusedField == .fontName
                        }
                        .onSubmit {
                            if let first = fontSuggestions.first {
                                isPickingFontSuggestion = true
                                settings.fontName = first
                                DispatchQueue.main.async {
                                    placeCaretAtEndOfFontField()
                                    isPickingFontSuggestion = false
                                }
                            }
                            showsFontSuggestions = false
                        }
                        .frame(width: 220, alignment: .leading)

                    Text("Installed font name")
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                        .lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .overlay(alignment: .topLeading) {
                    if showsFontSuggestions && !fontSuggestions.isEmpty {
                        fontSuggestionsDropdown
                            .offset(x: AppConstants.ThemeUI.labelWidth + 10, y: 30)
                    }
                }
                .zIndex(showsFontSuggestions ? 100 : 1)

                LabeledSlider(title: "Font Size", value: $settings.fontSize, range: 10...28)

                sectionHeader("Font Color")

                LabeledSlider(title: "Text Red", value: $settings.fontRed, range: 0...1)
                LabeledSlider(title: "Text Green", value: $settings.fontGreen, range: 0...1)
                LabeledSlider(title: "Text Blue", value: $settings.fontBlue, range: 0...1)
                LabeledSlider(title: "Text Opacity", value: $settings.fontOpacity, range: 0...1)

                sectionHeader("Border")

                LabeledSlider(title: "Border Thick", value: $settings.borderThickness, range: 0...6)
                LabeledSlider(title: "Border Red", value: $settings.borderRed, range: 0...1)
                LabeledSlider(title: "Border Green", value: $settings.borderGreen, range: 0...1)
                LabeledSlider(title: "Border Blue", value: $settings.fontBlue, range: 0...1)
                LabeledSlider(title: "Border Opacity", value: $settings.borderOpacity, range: 0...1)
            }
            .onAppear {
                focusedField = nil
            }
            .onChange(of: focusedField) { _, focused in
                if focused != .fontName {
                    showsFontSuggestions = false
                }
            }
            .onReceive(NotificationCenter.default.publisher(for: .lookFocusSettingsInputRequested)) { _ in
                DispatchQueue.main.async {
                    focusedField = .fontName
                    showsFontSuggestions = false
                }
            }
        }
    }

    private func sectionHeader(_ title: String) -> some View {
        HStack(spacing: 8) {
            Text("▶")
                .font(.system(size: CGFloat(settings.fontSize - 2)))
                .foregroundStyle(themeStore.secondaryTextColor())

            Text(title)
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                .foregroundStyle(themeStore.secondaryTextColor())

            Spacer(minLength: 0)
        }
    }

    @ViewBuilder
    private func sectionHeaderWithPicker<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        HStack(spacing: 8) {
            Text("▶")
                .font(.system(size: CGFloat(settings.fontSize - 2)))
                .foregroundStyle(themeStore.secondaryTextColor())

            Text(title)
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                .foregroundStyle(themeStore.secondaryTextColor())

            content()

            Spacer(minLength: 0)
        }
    }

    private func tabButton(title: String, index: Int) -> some View {
        let isActive = selectedTab == index
        return Button {
            selectedTab = index
            showsFontSuggestions = false
        } label: {
            Text(title)
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .medium))
                .foregroundStyle(isActive ? themeStore.fontColor() : themeStore.secondaryTextColor())
                .frame(maxWidth: .infinity)
                .padding(.vertical, 7)
                .background(
                    (isActive ? .white.opacity(0.16) : .white.opacity(0.06)),
                    in: RoundedRectangle(cornerRadius: 8, style: .continuous)
                )
        }
        .buttonStyle(.plain)
    }

    private func placeCaretAtEndOfFontField() {
        guard let editor = NSApp.keyWindow?.firstResponder as? NSTextView else {
            return
        }
        let location = (editor.string as NSString).length
        editor.setSelectedRange(NSRange(location: location, length: 0))
    }

    private var fontSuggestionsDropdown: some View {
        ScrollView(.vertical) {
            VStack(alignment: .leading, spacing: 2) {
                ForEach(fontSuggestions, id: \.self) { suggestion in
                    Button {
                        isPickingFontSuggestion = true
                        settings.fontName = suggestion
                        fontSuggestions = themeStore.fontNameSuggestions(for: suggestion, limit: 24)
                        showsFontSuggestions = false
                        DispatchQueue.main.async {
                            focusedField = .fontName
                            placeCaretAtEndOfFontField()
                            isPickingFontSuggestion = false
                        }
                    } label: {
                        Text(suggestion)
                            .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 6)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(4)
        }
        .frame(width: 240, height: 320, alignment: .topLeading)
        .scrollIndicators(.hidden)
        .background(.black.opacity(0.72), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .stroke(.white.opacity(0.12), lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.25), radius: 8, y: 4)
    }

    private var backgroundTab: some View {
        VStack(alignment: .leading, spacing: 10) {
            ScrollView(.vertical, showsIndicators: false) {
                VStack(alignment: .leading, spacing: 10) {
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

                }
            }
            .onAppear { syncIndexingInputsFromSettings() }
            .onChange(of: settings.fileScanDepth) { _, _ in
                fileScanDepthInput = String(settings.fileScanDepth)
            }
            .onChange(of: settings.fileScanLimit) { _, _ in
                fileScanLimitInput = String(settings.fileScanLimit)
            }

            Spacer(minLength: 0)

            Text(HintText.Settings.advancedApply)
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())
        }
    }

    private var shortcutsTab: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 14) {
                ForEach(ShortcutDocs.sections) { section in
                    ShortcutSection(title: section.title, items: section.items)
                }

                Text("This panel is intended as living documentation. We can add command and workflow docs here as features grow.")
                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())

                Text(HintText.Settings.shortcutsTips)
                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())
            }
            .padding(.top, 4)
        }
    }

    private func selectBackgroundImage() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.allowedContentTypes = [.image]
        if panel.runModal() == .OK {
            themeStore.setBackgroundImage(url: panel.url)
        }
    }

    private func selectExcludedFolderPath() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        if panel.runModal() == .OK, let url = panel.url {
            themeStore.addExcludedFolderPath(url: url)
        }
    }

    private func selectExtraScanDirectory() {
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

    private func syncIndexingInputsFromSettings() {
        fileScanDepthInput = String(settings.fileScanDepth)
        fileScanLimitInput = String(settings.fileScanLimit)
    }

    private func sanitizedNumericInput(_ value: String) -> String {
        String(value.filter(\.isNumber))
    }

    private func applyFileScanDepthInput() {
        guard let parsed = Int(fileScanDepthInput), parsed > 0 else {
            fileScanDepthInput = String(settings.fileScanDepth)
            return
        }
        settings.fileScanDepth = min(max(1, parsed), 12)
        fileScanDepthInput = String(settings.fileScanDepth)
    }

    private func applyFileScanLimitInput() {
        guard let parsed = Int(fileScanLimitInput), parsed > 0 else {
            fileScanLimitInput = String(settings.fileScanLimit)
            return
        }
        settings.fileScanLimit = min(max(500, parsed), 50_000)
        fileScanLimitInput = String(settings.fileScanLimit)
    }

    private func closeSettingsPanel() {
        appUIState.showsThemeSettings = false
        NotificationCenter.default.post(name: .lookRefocusInputRequested, object: nil)
    }

    private func installLocalKeyMonitorIfNeeded() {
        guard localKeyMonitor == nil else { return }

        localKeyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)

            if event.keyCode == 53 && flags.isEmpty {
                closeSettingsPanel()
                return nil
            }

            if showFreshConfigConfirm,
               flags.isEmpty,
               event.charactersIgnoringModifiers?.lowercased() == "y" {
                showFreshConfigConfirm = false
                runFreshConfigReset()
                return nil
            }

            if flags == [.command, .shift]
                && (event.charactersIgnoringModifiers == "," || event.keyCode == 43)
            {
                closeSettingsPanel()
                return nil
            }

            return event
        }
    }

    private func removeLocalKeyMonitor() {
        guard let localKeyMonitor else { return }
        NSEvent.removeMonitor(localKeyMonitor)
        self.localKeyMonitor = nil
    }

    private var hasIndexingError: Bool {
        fileScanDepthError != nil || fileScanLimitError != nil
    }

    private func runFreshConfigReset() {
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
}

private struct ShortcutItem: Identifiable {
    let id = UUID()
    let keys: String
    let action: String
}

private struct ShortcutSectionData: Identifiable {
    let id = UUID()
    let title: String
    let items: [ShortcutItem]
}

private enum ShortcutDocs {
    static let sections: [ShortcutSectionData] = [
        ShortcutSectionData(
            title: "Core launcher",
            items: [
                ShortcutItem(keys: "Tab", action: "Select next result"),
                ShortcutItem(keys: "Shift+Tab", action: "Select previous result"),
                ShortcutItem(keys: "Up / Down", action: "Move selection"),
                ShortcutItem(keys: "Cmd+C", action: "Copy selected file/folder to pasteboard"),
                ShortcutItem(keys: "Cmd+F", action: "Reveal selected app/file/folder in Finder"),
                ShortcutItem(keys: "Cmd+Enter", action: "Search query on Google"),
                ShortcutItem(keys: "Cmd+/", action: "Enter command mode"),
                ShortcutItem(keys: "Cmd+Shift+,", action: "Open/close settings panel"),
                ShortcutItem(keys: "Cmd+Shift+;", action: "Reload .look.config"),
                ShortcutItem(keys: "Cmd+H", action: "Toggle in-window keyboard help screen"),
                ShortcutItem(keys: "Esc", action: "Back to app list (in command mode)"),
                ShortcutItem(keys: "Shift+Esc", action: "Hide launcher"),
            ]
        ),
        ShortcutSectionData(
            title: "Search prefixes",
            items: [
                ShortcutItem(keys: "a\"", action: "Apps-only query"),
                ShortcutItem(keys: "f\"", action: "Files-only query"),
                ShortcutItem(keys: "d\"", action: "Folders-only query"),
                ShortcutItem(keys: "r\"", action: "Regex query"),
                ShortcutItem(keys: "c\"", action: "Clipboard history query"),
            ]
        ),
        ShortcutSectionData(
            title: "Clipboard history",
            items: [
                ShortcutItem(keys: "Enter", action: "Copy selected history item back to clipboard"),
                ShortcutItem(keys: "Delete button", action: "Remove selected clipboard item from look history"),
            ]
        ),
        ShortcutSectionData(
            title: "Panels",
            items: [
                ShortcutItem(keys: "Cmd+Shift+,", action: "Open/close theme and docs panel"),
                ShortcutItem(keys: "Cmd+Shift+;", action: "Reload .look.config"),
                ShortcutItem(keys: "Save Config", action: "Write current UI settings to .look.config"),
            ]
        ),
        ShortcutSectionData(
            title: "Zoom",
            items: [
                ShortcutItem(keys: "Cmd+-", action: "Zoom out UI scale"),
                ShortcutItem(keys: "Cmd+=", action: "Zoom in UI scale"),
                ShortcutItem(keys: "Cmd+0", action: "Reset UI scale"),
            ]
        ),
        ShortcutSectionData(
            title: "Theme controls",
            items: [
                ShortcutItem(keys: "Appearance tab", action: "Tint, blur material, blur opacity"),
                ShortcutItem(keys: "Advanced tab", action: "Background, indexing, privacy, logging controls"),
                ShortcutItem(keys: "Shortcuts tab", action: "In-app keyboard documentation"),
            ]
        ),
    ]
}

private struct ShortcutSection: View {
    @EnvironmentObject private var themeStore: ThemeStore

    let title: String
    let items: [ShortcutItem]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .medium))
                .foregroundStyle(themeStore.secondaryTextColor())

            ForEach(items) { item in
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    Text(item.keys)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(.white.opacity(0.14), in: Capsule())
                    Text(item.action)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.secondaryTextColor())
                    Spacer(minLength: 0)
                }
            }
        }
    }
}

private struct LabeledSlider: View {
    @EnvironmentObject private var themeStore: ThemeStore

    let title: String
    @Binding var value: Double
    let range: ClosedRange<Double>

    private var valueColumnWidth: CGFloat {
        let scaledFontSize = CGFloat(themeStore.settings.fontSize) * themeStore.uiScale
        return max(42, scaledFontSize * 2.3 + 14)
    }

    var body: some View {
        HStack(spacing: 10) {
            Text(title)
                .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())
            Slider(value: $value, in: range)
                .controlSize(.mini)
                .tint(themeStore.fontColor(opacityMultiplier: 0.92))
            Text(value, format: .number.precision(.fractionLength(2)))
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .monospacedDigit()
                .lineLimit(1)
                .frame(width: valueColumnWidth, alignment: .trailing)
                .foregroundStyle(themeStore.mutedTextColor())
        }
    }
}
