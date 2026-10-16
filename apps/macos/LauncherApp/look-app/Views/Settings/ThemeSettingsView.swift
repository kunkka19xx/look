import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct ThemeSettingsView: View {
    enum Field {
        case fontName
    }

    /// Outcome carried with the text so a failed save cannot be styled as a
    /// success by a stale flag.
    struct SaveMessage {
        let text: String
        let succeeded: Bool
    }

    static let activeTabFillOpacity = 0.16
    static let inactiveTabFillOpacity = 0.06
    /// The floating Save button's distance from the panel's bottom right.
    static let saveFloatInset: CGFloat = 10

    @EnvironmentObject var appUIState: AppUIState
    @EnvironmentObject var themeStore: ThemeStore
    @ObservedObject var updateChecker = UpdateChecker.shared
    @Binding var settings: ThemeSettings
    @State var selectedTab = 0
    @State var saveMessage: SaveMessage?
    @State var fontSuggestions: [String] = []
    @State var showsFontSuggestions = false
    @State var isPickingFontSuggestion = false
    @State var fileScanDepthInput = ""
    @State var fileScanLimitInput = ""
    @State var fileScanDepthError: String?
    @State var fileScanLimitError: String?
    @State var extraScanDirectoryMessage: String?
    @State var showFreshConfigConfirm = false
    @State var freshConfigMessage: String?
    @State var localKeyMonitor: Any?
    @FocusState var focusedField: Field?

    var body: some View {
        // No title and no close hint: the tabs say where you are, and the row
        // they replace is a row of settings the panel can show instead.
        VStack(alignment: .leading, spacing: 12) {
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
        .overlay(alignment: .bottomTrailing) {
            saveControls
        }
        .onExitCommand {
            closeSettingsPanel()
        }
        // Nothing is focused on open. Otherwise the window hands first
        // responder to whichever text field comes first, and a stray keystroke
        // silently rewrites a live value (the Ollama host learned this the
        // hard way).
        .defaultFocus($focusedField, nil)
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

    /// Save floats at the bottom right rather than taking a row of its own:
    /// the button sits where the eye ends, and the screen keeps the space.
    private var saveControls: some View {
        HStack(spacing: 10) {
            if let saveMessage {
                Text(saveMessage.text)
                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                    .foregroundStyle(saveMessage.succeeded ? themeStore.onSuccessColor() : themeStore.onDangerColor())
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(
                        saveMessage.succeeded ? themeStore.successColor() : themeStore.dangerColor(),
                        in: Capsule()
                    )
            }

            Button("Save Config") {
                applyFileScanDepthInput()
                applyFileScanLimitInput()
                let ok = themeStore.saveCurrentConfigToFile()
                saveMessage = SaveMessage(text: ok ? "Saved" : "Save failed", succeeded: ok)
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
        }
        .padding(Self.saveFloatInset)
    }

    /// One settings row: label column, control column, then the hint. The two
    /// fixed columns are what keep every control and every hint on one x.
    func settingRow<Control: View, Trailing: View>(
        _ label: String?,
        hint: String? = nil,
        alignment: VerticalAlignment = .center,
        @ViewBuilder control: () -> Control,
        @ViewBuilder trailing: () -> Trailing = { EmptyView() }
    ) -> some View {
        HStack(alignment: alignment, spacing: 10) {
            Text(label ?? "")
                .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())

            control()
                .frame(width: AppConstants.ThemeUI.controlWidth, alignment: .leading)

            if let hint {
                Text(hint)
                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
                    .lineLimit(1)
            }

            trailing()

            Spacer(minLength: 0)
        }
    }

    func sectionHeader(_ title: String) -> some View {
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
    func sectionHeaderWithPicker<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
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

    func tabButton(title: String, index: Int) -> some View {
        let isActive = selectedTab == index
        let tabCornerRadius = themeStore.controlRadius
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
                    themeStore.liftColor(opacity: isActive ? Self.activeTabFillOpacity : Self.inactiveTabFillOpacity),
                    in: RoundedRectangle(cornerRadius: tabCornerRadius, style: .continuous)
                )
        }
        .buttonStyle(.plain)
    }

    func closeSettingsPanel() {
        appUIState.showsThemeSettings = false
        NotificationCenter.default.post(name: .lookRefocusInputRequested, object: nil)
    }

    func installLocalKeyMonitorIfNeeded() {
        guard localKeyMonitor == nil else { return }

        localKeyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if ShortcutCapture.isActive { return event }
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

            // ⌘⇧C reaches in here too, so a layout can be tried on from the
            // picker, which keeps showing the saved one and rings this.
            if flags == [.command, .shift]
                && (event.charactersIgnoringModifiers?.lowercased() == "c"
                    || event.keyCode == AppConstants.Launcher.KeyCode.c)
            {
                themeStore.toggleSessionLayout()
                return nil
            }

            return event
        }
    }

    func removeLocalKeyMonitor() {
        guard let localKeyMonitor else { return }
        NSEvent.removeMonitor(localKeyMonitor)
        self.localKeyMonitor = nil
    }
}

struct LabeledSlider: View {
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
