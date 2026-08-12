import SwiftUI

/// The AI block of Advanced settings, extracted into its own `View` so the giant
/// `backgroundTab` body stays small. (A single over-large @ViewBuilder body could
/// miscompile into an AttributeGraph crash when the settings scroll view updates.)
struct AISettingsSection: View {
    @EnvironmentObject var themeStore: ThemeStore
    @Binding var settings: ThemeSettings

    private var availability: AIProviderAvailability {
        AIQueryRouter.shared.availability(of: settings.aiProvider)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            headerRow
            providerRow

            if settings.aiProvider == .ollama {
                ollamaRow
            }

            // Only worth a row when the configured provider is off-machine;
            // with a local model there is nothing to decide.
            if settings.aiEnabled, !AIQueryRouter.shared.allowsPrivateContext(settings.aiProvider)
                || settings.aiAllowRemoteContext {
                remoteContextRow
            }

            PermissionsRow(themeStore: themeStore)
        }
    }

    private var headerRow: some View {
        HStack(spacing: 10) {
            Text("AI")
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .semibold))
                .foregroundStyle(themeStore.secondaryTextColor())

            // Not gated on provider availability - the web answer card and search
            // suggestions work without it; the model tier self-skips when off.
            Toggle("Enable AI & web answers", isOn: $settings.aiEnabled)
                .toggleStyle(.switch)
                .labelsHidden()
                .help("Show instant answers and web search suggestions (sends queries to the web; opt-out anytime)")

            infoIndicator

            Spacer(minLength: 0)
        }
    }

    private var providerRow: some View {
        HStack(spacing: 10) {
            Text("Provider")
                .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())

            Picker("AI provider", selection: $settings.aiProvider) {
                ForEach(AIProviderKind.allCases) { kind in
                    Text(kind.title).tag(kind)
                }
            }
            .labelsHidden()
            .frame(maxWidth: 260, alignment: .leading)

            Spacer(minLength: 0)
        }
    }

    private var ollamaRow: some View {
        HStack(spacing: 10) {
            Text("Ollama")
                .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())
            TextField("http://localhost:11434", text: $settings.ollamaHost)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 180)
            OllamaModelField(
                host: settings.ollamaHost,
                model: $settings.ollamaModel,
                themeStore: themeStore)
                .frame(maxWidth: 160)
            Spacer(minLength: 0)
        }
    }

    /// The provider isn't on this machine, so personal context is withheld
    /// unless the user says otherwise. Never a per-query prompt: disclosure is
    /// a policy, and re-asking it would just train people to click through.
    private var remoteContextRow: some View {
        HStack(spacing: 10) {
            Text("Personal context")
                .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())
            Toggle("Send to remote model", isOn: $settings.aiAllowRemoteContext)
                .toggleStyle(.switch)
                .help(
                    "This provider is not on your Mac. Off keeps your calendar, "
                        + "clipboard, and remembered facts out of its prompts.")
            Spacer(minLength: 0)
        }
    }

    private var infoIndicator: some View {
        Image(systemName: availability.isAvailable ? "checkmark.circle.fill" : "info.circle")
            .font(.system(size: CGFloat(settings.fontSize)))
            .foregroundStyle(
                availability.isAvailable
                    ? Color.green.opacity(0.85)
                    : themeStore.secondaryTextColor())
            .contentShape(Rectangle())
            .accessibilityLabel(Text("AI availability"))
            .hoverTooltip(tooltip)
    }

    private var tooltip: String {
        let base = "Shows instant answers (facts, definitions, weather, currency, "
            + "crypto, calculations) and web search suggestions for your queries. "
            + "Powered by free web sources (Wikipedia, DuckDuckGo) plus the "
            + "selected AI provider where available. Queries are sent to the web "
            + "while this is on."
        let providerName = settings.aiProvider.title
        switch availability {
        case .available:
            return base + "\n\n\(providerName): Ready."
        case .unavailable(let reason):
            return base + "\n\n\(providerName): \(reason.userFacingMessage) "
                + "(Web answers and suggestions still work without it.)"
        }
    }
}
