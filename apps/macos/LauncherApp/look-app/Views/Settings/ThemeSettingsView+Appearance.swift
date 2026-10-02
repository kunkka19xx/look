import AppKit
import SwiftUI

extension ThemeSettingsView {
    var appearanceTab: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 14) {
                    inlinePickerLabel("Theme")
                    Picker("Theme", selection: $settings.uiTheme) {
                        ForEach(BuiltinThemePreset.options(including: settings.uiTheme)) { preset in
                            Text(preset.pickerTitle).tag(preset)
                        }
                    }
                    .pickerStyle(.menu)
                    .labelsHidden()
                    .frame(width: AppConstants.ThemeUI.pickerWidth)
                    .onChange(of: settings.uiTheme) { _, newValue in
                        themeStore.applyBuiltinTheme(newValue)
                    }

                    Spacer(minLength: 0)
                }

                HStack(spacing: 14) {
                    // Compact shows neither the strip nor the launchpad, so
                    // the session override (⌘⇧C) hides these too.
                    if themeStore.effectiveLayout == .split {
                        appearanceSwitch(
                            "Running Apps",
                            isOn: Binding(
                                get: { settings.runningAppsPlacement != .none },
                                set: { settings.runningAppsPlacement = $0 ? .right : .none }
                            ),
                            help: "Show running apps in the right half of the search bar (⌘1-9 to switch)")

                        if settings.runningAppsPlacement != .none {
                            Spacer().frame(width: 40)

                            appearanceSwitch(
                                "Theme Tint",
                                isOn: $settings.runningAppsThemeTint,
                                help: "Discreetly tint open app icons to match the active theme palette")
                        }

                        Spacer().frame(width: 40)

                        appearanceSwitch(
                            "Super Actions",
                            isOn: $settings.superActionsEnabled,
                            help: "Show the quick-actions launchpad on the empty home screen (⌘ + letter)")

                        Spacer().frame(width: 40)
                    }

                    appearanceSwitch(
                        "Animations",
                        isOn: $settings.animationsEnabled,
                        help: "Animate the launcher when it opens and as the selection moves. Off shows every change instantly.")

                    Spacer(minLength: 0)
                }

                Divider()
                    .overlay(themeStore.dividerColor())
                    .padding(.vertical, 4)

                searchBarLivePreview

                sectionHeader("Layout")

                HStack(spacing: 10) {
                    Text("Window")
                        .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.secondaryTextColor())

                    layoutSegment

                    Spacer(minLength: 0)
                }

                LabeledSlider(
                    title: "Content Width",
                    value: $settings.windowWidth,
                    range: AppConstants.ThemeUI.windowWidthRange,
                    step: 10,
                    fractionLength: 0)
                    .help("Width of the results and preview content in points (500–1400; default 860).")

                LabeledSlider(
                    title: "Search Bar Width",
                    value: $settings.searchBarWidth,
                    range: AppConstants.ThemeUI.searchBarWidthRange,
                    step: 10,
                    fractionLength: 0)
                    .help("Width of the search bar in points (350–1400; default 860). Can be narrower than the content.")

                LabeledSlider(
                    title: "Inner Gap",
                    value: $settings.innerGap,
                    range: AppConstants.ThemeUI.innerGapRange)
                    .help("i3-style gap between the top row, results list and preview. 0 = flat layout; higher turns each into its own card.")

                LabeledSlider(
                    title: "Corner Radius",
                    value: $settings.surfaceRadius,
                    range: AppConstants.ThemeUI.surfaceRadiusRange)
                    .help("Corner rounding, shared by every surface: the panel, the top bar, the launchpad tiles and the controls. 0 = square.")

                sectionHeader("Bar & Background Color")

                colorSelectionRow

                if showsRgbSliders {
                    LabeledSlider(title: "Red", value: $settings.tintRed, range: 0...1)
                    LabeledSlider(title: "Green", value: $settings.tintGreen, range: 0...1)
                    LabeledSlider(title: "Blue", value: $settings.tintBlue, range: 0...1)
                }

                LabeledSlider(
                    title: "Tint Opacity",
                    value: $settings.tintOpacity,
                    range: 0...1)
                    .help("How strongly the color covers the frosted backdrop (0 = pure blur, 1 = solid color).")

                sectionHeader("Blur")

                HStack(spacing: 10) {
                    Text("Blur Style")
                        .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.secondaryTextColor())

                    Picker("Blur Style", selection: $settings.blurMaterial) {
                        ForEach(LauncherBlurMaterial.options(including: settings.blurMaterial)) { item in
                            Text(item.pickerTitle).tag(item)
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

                // Disabled rather than hidden, to keep the value visible.
                LabeledSlider(title: "Blur Opacity", value: $settings.blurOpacity, range: 0...1)
                    .disabled(settings.blurMaterial.rendersGlass)
                    .opacity(settings.blurMaterial.rendersGlass ? AppConstants.ThemeUI.disabledControlOpacity : 1)
                    .help("Strength of the background frost blur.")

                HStack(spacing: 10) {
                    LabeledSlider(title: "Settings Blur", value: $settings.settingsBlurMultiplier, range: 0.4...1)
                        .help("How much the backdrop thins while Settings is open. Set to 1.0 to match the launcher.")

                    if abs(settings.settingsBlurMultiplier - 1.0) > 0.01 {
                        Button("Match Launcher (1.0)") {
                            settings.settingsBlurMultiplier = 1.0
                        }
                        .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                        .buttonStyle(.plain)
                        .foregroundStyle(themeStore.accentColor())
                    }
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
                LabeledSlider(title: "Border Blue", value: $settings.borderBlue, range: 0...1)
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

    /// Window layout. The fill is the saved layout; ⌘⇧C can leave the window in
    /// the other one for the rest of the run, and that one takes the ring.
    var layoutSegment: some View {
        let live = themeStore.effectiveLayout
        let itemRadius = max(0, themeStore.controlRadius - 2)
        return HStack(spacing: 2) {
            ForEach(LauncherLayout.allCases) { option in
                let saved = settings.layout == option
                Button {
                    settings.layout = option
                } label: {
                    Text(option.title)
                        .font(themeStore.uiFont(
                            size: CGFloat(settings.fontSize - 1),
                            weight: saved ? .semibold : .regular))
                        .foregroundStyle(saved ? themeStore.onAccentColor() : themeStore.secondaryTextColor())
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 3)
                        .background(
                            saved ? themeStore.accentColor() : Color.clear,
                            in: RoundedRectangle(cornerRadius: itemRadius, style: .continuous)
                        )
                        .overlay {
                            if !saved && live == option {
                                RoundedRectangle(cornerRadius: itemRadius, style: .continuous)
                                    .stroke(themeStore.accentColor(), lineWidth: 1)
                            }
                        }
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(2)
        .frame(width: AppConstants.ThemeUI.pickerWidth)
        .background(
            themeStore.liftColor(opacity: 0.06),
            in: RoundedRectangle(cornerRadius: themeStore.controlRadius, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: themeStore.controlRadius, style: .continuous)
                .stroke(themeStore.dividerColor(), lineWidth: 1)
        )
        .help("Split shows results beside a preview; Compact is a smaller window with results only. ⌘⇧C switches for this session only, and rings the one in use.")
    }

    private var suggestionCornerRadius: CGFloat {
        themeStore.controlRadius
    }

    var fontSuggestionsDropdown: some View {
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
        .background(
            themeStore.scrimColor(opacity: 0.72),
            in: RoundedRectangle(cornerRadius: suggestionCornerRadius, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: suggestionCornerRadius, style: .continuous)
                .stroke(themeStore.liftColor(opacity: 0.12), lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.25), radius: 8, y: 4)
    }

    @ViewBuilder
    func inlinePickerLabel(_ title: String) -> some View {
        HStack(spacing: 6) {
            Text("▶")
                .font(.system(size: CGFloat(settings.fontSize - 2)))
                .foregroundStyle(themeStore.secondaryTextColor())
            Text(title)
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                .foregroundStyle(themeStore.secondaryTextColor())
        }
    }

    /// A labelled switch; the label keeps one line so a narrow row cannot wrap it.
    func appearanceSwitch(_ title: String, isOn: Binding<Bool>, help: String) -> some View {
        HStack(spacing: 14) {
            inlinePickerLabel(title)
                .fixedSize()
            Toggle(title, isOn: isOn)
                .toggleStyle(.switch)
                .labelsHidden()
                .help(help)
        }
    }

    func placeCaretAtEndOfFontField() {
        guard let editor = NSApp.keyWindow?.firstResponder as? NSTextView else {
            return
        }
        let location = (editor.string as NSString).length
        editor.setSelectedRange(NSRange(location: location, length: 0))
    }

    // MARK: - Search Bar Live Preview & Color Helpers

    var searchBarLivePreview: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("Search Bar Live Preview")
                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .semibold))
                    .foregroundStyle(themeStore.secondaryTextColor())
                Spacer()
                Text("\(Int(settings.searchBarWidth)) pt")
                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
            }

            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(themeStore.secondaryTextColor())
                    .frame(width: 16, height: 16)

                Text(AppConstants.Launcher.searchPlaceholder)
                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize)))
                    .foregroundStyle(themeStore.placeholderTextColor())
                    .lineLimit(1)

                Spacer(minLength: 8)

                if settings.runningAppsPlacement != .none {
                    HStack(spacing: 5) {
                        Image(systemName: "applelogo")
                            .font(.system(size: 11))
                            .foregroundStyle(themeStore.fontColor().opacity(0.7))
                            .padding(4)
                            .background(themeStore.liftColor(opacity: 0.12), in: RoundedRectangle(cornerRadius: 4))
                        Image(systemName: "terminal")
                            .font(.system(size: 11))
                            .foregroundStyle(themeStore.accentColor())
                            .padding(4)
                            .background(themeStore.liftColor(opacity: 0.12), in: RoundedRectangle(cornerRadius: 4))
                    }
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .frame(maxWidth: min(CGFloat(settings.searchBarWidth), 520))
            .background {
                ThemedBackdrop(
                    themeStore: themeStore,
                    blurOpacityMultiplier: 1.0,
                    blendingMode: .withinWindow,
                    cornerRadius: themeStore.barRadius
                )
            }
            .clipShape(RoundedRectangle(cornerRadius: themeStore.barRadius, style: .continuous))
            .overlay {
                if themeStore.borderLineWidth() > 0 {
                    RoundedRectangle(cornerRadius: themeStore.barRadius, style: .continuous)
                        .strokeBorder(themeStore.borderColor(), lineWidth: themeStore.borderLineWidth())
                }
            }
            .shadow(color: .black.opacity(0.22), radius: 6, x: 0, y: 3)
            .frame(maxWidth: .infinity, alignment: .center)
            .padding(.vertical, 4)
        }
        .padding(10)
        .background(
            themeStore.liftColor(opacity: 0.05),
            in: RoundedRectangle(cornerRadius: themeStore.controlRadius, style: .continuous)
        )
    }

    var colorSelectionRow: some View {
        HStack(spacing: 10) {
            Text("Color")
                .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())

            ColorPicker("", selection: backgroundColorBinding, supportsOpacity: false)
                .labelsHidden()
                .frame(width: 28, height: 24)

            Text(hexColorString)
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .medium))
                .monospaced()
                .foregroundStyle(themeStore.fontColor())
                .padding(.horizontal, 6)
                .padding(.vertical, 3)
                .background(themeStore.liftColor(opacity: 0.12), in: RoundedRectangle(cornerRadius: 4))

            HStack(spacing: 6) {
                ForEach(colorPresets) { preset in
                    Button {
                        settings.tintRed = preset.red
                        settings.tintGreen = preset.green
                        settings.tintBlue = preset.blue
                    } label: {
                        Circle()
                            .fill(preset.color)
                            .frame(width: 16, height: 16)
                            .overlay {
                                Circle()
                                    .strokeBorder(themeStore.fontColor().opacity(0.25), lineWidth: 1)
                            }
                    }
                    .buttonStyle(.plain)
                    .help(preset.name)
                }
            }

            Spacer(minLength: 0)

            Button {
                showsRgbSliders.toggle()
            } label: {
                HStack(spacing: 3) {
                    Text(showsRgbSliders ? "Hide RGB" : "RGB Sliders")
                    Image(systemName: showsRgbSliders ? "chevron.up" : "chevron.down")
                }
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())
            }
            .buttonStyle(.plain)
        }
    }

    var backgroundColorBinding: Binding<Color> {
        Binding(
            get: {
                Color(
                    .sRGB,
                    red: settings.tintRed,
                    green: settings.tintGreen,
                    blue: settings.tintBlue,
                    opacity: 1.0
                )
            },
            set: { newColor in
                let nsColor = NSColor(newColor).usingColorSpace(.sRGB) ?? NSColor(newColor)
                settings.tintRed = max(0, min(1, Double(nsColor.redComponent)))
                settings.tintGreen = max(0, min(1, Double(nsColor.greenComponent)))
                settings.tintBlue = max(0, min(1, Double(nsColor.blueComponent)))
            }
        )
    }

    var hexColorString: String {
        let r = Int(round(settings.tintRed * 255))
        let g = Int(round(settings.tintGreen * 255))
        let b = Int(round(settings.tintBlue * 255))
        return String(format: "#%02X%02X%02X", r, g, b)
    }

    struct ColorPreset: Identifiable {
        let id: String
        let name: String
        let red: Double
        let green: Double
        let blue: Double

        var color: Color {
            Color(.sRGB, red: red, green: green, blue: blue, opacity: 1.0)
        }
    }

    var colorPresets: [ColorPreset] {
        [
            ColorPreset(id: "obsidian", name: "Obsidian Black", red: 0.0, green: 0.0, blue: 0.0),
            ColorPreset(id: "slate", name: "Slate Gray", red: 0.08, green: 0.10, blue: 0.14),
            ColorPreset(id: "navy", name: "Deep Navy", red: 0.06, green: 0.09, blue: 0.18),
            ColorPreset(id: "forest", name: "Dark Forest", red: 0.06, green: 0.12, blue: 0.10),
            ColorPreset(id: "plum", name: "Midnight Plum", red: 0.14, green: 0.08, blue: 0.16),
            ColorPreset(id: "snow", name: "Frosted Snow", red: 0.95, green: 0.95, blue: 0.97),
        ]
    }
}
