import AppKit
import SwiftUI

extension ThemeSettingsView {
    var appearanceTab: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 10) {
                sectionHeader(Self.launcherSection)

                settingRow(Self.themeLabel) {
                    Picker(Self.themeLabel, selection: $settings.uiTheme) {
                        ForEach(BuiltinThemePreset.options(including: settings.uiTheme)) { preset in
                            Text(preset.pickerTitle).tag(preset)
                        }
                    }
                    .settingsMenuPicker()
                    .onChange(of: settings.uiTheme) { _, newValue in
                        themeStore.applyBuiltinTheme(newValue)
                    }
                }

                // Compact has no bar slot and no launchpad, so their rows are split only.
                if themeStore.effectiveLayout == .split {
                    settingRow(Self.barSlotLabel) {
                        Picker(Self.barSlotLabel, selection: $settings.barSlot) {
                            ForEach(BarSlot.allCases) { slot in
                                Text(slot.title).tag(slot)
                            }
                        }
                        .settingsMenuPicker()
                        .help(Self.barSlotHelp)
                    }

                    switch settings.barSlot {
                    case .pinned:
                        pinnedAppsRow
                    case .script:
                        barScriptRow
                    case .running, .todo, .none:
                        EmptyView()
                    }
                }

                switchesRow

                Divider()
                    .overlay(themeStore.dividerColor())
                    .padding(.vertical, 4)

                sectionHeader("Layout")

                settingRow("Window", controlWidth: nil) {
                    layoutSegment
                }

                LabeledSlider(
                    title: "Inner Gap",
                    value: $settings.innerGap,
                    range: AppConstants.ThemeUI.innerGapRange
                )
                .help(
                    "i3-style gap between the top row, results list and preview. 0 = flat layout; higher turns each into its own card."
                )

                LabeledSlider(
                    title: "Corner Radius",
                    value: $settings.surfaceRadius,
                    range: AppConstants.ThemeUI.surfaceRadiusRange
                )
                .help(
                    "Corner rounding, shared by every surface: the panel, the top bar, the launchpad tiles and the controls. 0 = square."
                )

                sectionHeader("Tint Color")

                LabeledSlider(title: "Red", value: $settings.tintRed, range: 0...1)
                LabeledSlider(title: "Green", value: $settings.tintGreen, range: 0...1)
                LabeledSlider(title: "Blue", value: $settings.tintBlue, range: 0...1)
                LabeledSlider(title: "Tint Opacity", value: $settings.tintOpacity, range: 0...1)

                sectionHeader("Blur")

                // Disabled rather than hidden, to keep the value visible.
                LabeledSlider(title: "Blur Opacity", value: $settings.blurOpacity, range: 0...1)
                    .disabled(settings.blurMaterial.rendersGlass)
                    .opacity(settings.blurMaterial.rendersGlass ? AppConstants.ThemeUI.disabledControlOpacity : 1)

                LabeledSlider(title: "Settings Blur", value: $settings.settingsBlurMultiplier, range: 0.4...1)
                    .help("How much the backdrop thins while Settings is open.")

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
                    .settingsMenuPicker()

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
                        .font(
                            themeStore.uiFont(
                                size: CGFloat(settings.fontSize - 1),
                                weight: saved ? .semibold : .regular)
                        )
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
        .help(
            "Split shows results beside a preview; Compact is a smaller window with results only. ⌘⇧C switches for this session only, and rings the one in use."
        )
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

    private struct LauncherSwitch {
        let title: String
        let isOn: WritableKeyPath<ThemeSettings, Bool>
        let help: String
    }

    private static let launcherSection = "Launcher"
    private static let themeLabel = "Theme"
    private static let switchGap: CGFloat = 30
    private static let switchLabelGap: CGFloat = 10
    private static let superActionsSwitch = LauncherSwitch(
        title: "Super Actions", isOn: \.superActionsEnabled,
        help: "Show the quick-actions launchpad on the empty home screen (⌘ + letter)")
    private static let animationsSwitch = LauncherSwitch(
        title: "Animations", isOn: \.animationsEnabled,
        help: "Animate the launcher when it opens and as the selection moves. Off shows every change instantly.")
    private static let christmasSwitch = LauncherSwitch(
        title: "Christmas", isOn: \.christmasDecorations,
        help:
            "Falling snow, glowing lights behind the panels, a tree behind the results and a snowman search icon, over any theme."
    )

    /// One line: the first switch in the label column, the rest after it.
    private var switchesRow: some View {
        let switches =
            themeStore.effectiveLayout == .split
            ? [Self.superActionsSwitch, Self.animationsSwitch, Self.christmasSwitch]
            : [Self.animationsSwitch, Self.christmasSwitch]
        return settingRow(switches[0].title, controlWidth: nil) {
            HStack(spacing: Self.switchGap) {
                launcherToggle(switches[0])
                ForEach(switches.dropFirst(), id: \.title) { item in
                    HStack(spacing: Self.switchLabelGap) {
                        settingLabel(item.title)
                            .fixedSize()
                        launcherToggle(item)
                    }
                }
                if settings.christmasDecorations {
                    Picker("Snow", selection: $settings.christmasSnowAmount) {
                        ForEach(AppConstants.ThemeUI.snowAmounts, id: \.self) { amount in
                            Text("Snow \(amount.formatted())x").tag(amount)
                        }
                    }
                    .pickerStyle(.menu)
                    .labelsHidden()
                    .fixedSize()
                    .help("How much snow falls")
                }
            }
        }
    }

    private func launcherToggle(_ item: LauncherSwitch) -> some View {
        Toggle(item.title, isOn: $settings[dynamicMember: item.isOn])
            .toggleStyle(.switch)
            .labelsHidden()
            .help(item.help)
    }

    func placeCaretAtEndOfFontField() {
        guard let editor = NSApp.keyWindow?.firstResponder as? NSTextView else {
            return
        }
        let location = (editor.string as NSString).length
        editor.setSelectedRange(NSRange(location: location, length: 0))
    }
}

extension View {
    /// A menu picker hugs its title, so it is pinned to the control column's edge.
    fileprivate func settingsMenuPicker() -> some View {
        pickerStyle(.menu)
            .labelsHidden()
            .frame(width: AppConstants.ThemeUI.pickerWidth, alignment: .leading)
    }
}
