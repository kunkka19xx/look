import AppKit
import SwiftUI

extension ThemeSettingsView {
    var appearanceTab: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 14) {
                    inlinePickerLabel("Theme")
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

                    Spacer().frame(width: 40)

                    inlinePickerLabel("Running Apps")
                    Toggle("Show running apps", isOn: Binding(
                        get: { settings.runningAppsPlacement != .none },
                        set: { settings.runningAppsPlacement = $0 ? .right : .none }
                    ))
                    .toggleStyle(.switch)
                    .labelsHidden()
                    .help("Show running apps in the right half of the search bar (⌘1-9 to switch)")

                    Spacer(minLength: 0)
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
        .background(.black.opacity(0.72), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .stroke(.white.opacity(0.12), lineWidth: 1)
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

    func placeCaretAtEndOfFontField() {
        guard let editor = NSApp.keyWindow?.firstResponder as? NSTextView else {
            return
        }
        let location = (editor.string as NSString).length
        editor.setSelectedRange(NSRange(location: location, length: 0))
    }
}
