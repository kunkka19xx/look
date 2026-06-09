import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct ThemeSettingsView: View {
    enum Field {
        case fontName
    }

    @EnvironmentObject var appUIState: AppUIState
    @EnvironmentObject var themeStore: ThemeStore
    @ObservedObject var updateChecker = UpdateChecker.shared
    @Binding var settings: ThemeSettings
    @State var selectedTab = 0
    @State var saveMessage: String?
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

    func closeSettingsPanel() {
        appUIState.showsThemeSettings = false
        NotificationCenter.default.post(name: .lookRefocusInputRequested, object: nil)
    }

    func installLocalKeyMonitorIfNeeded() {
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
