import SwiftUI

/// The Ollama endpoint: type it, or pick one look found running. Same shape as
/// the font field (free text with a suggestion list underneath) rather than the
/// model field's menu, because a host cannot be enumerated - only the usual
/// candidates can be probed, and a remote host still has to be typed.
///
/// It also deliberately does NOT take focus on its own. This field used to be
/// the window's first responder, one keystroke from replacing a working URL
/// with nothing, which is exactly how a blank host once turned into "the model
/// returned no answer".
struct OllamaHostField: View {
    @Binding var host: String
    let themeStore: ThemeStore

    /// Where a local daemon actually listens. Probed, never assumed.
    private static let candidates = [
        "http://localhost:11434",
        "http://127.0.0.1:11434",
    ]

    @State private var reachable: [String] = []
    @State private var showsSuggestions = false
    @State private var probeTask: Task<Void, Never>?
    @FocusState private var focused: Bool

    private var suggestions: [String] {
        let typed = host.trimmingCharacters(in: .whitespaces).lowercased()
        return reachable.filter { $0 != host && (typed.isEmpty || $0.lowercased().contains(typed)) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            TextField("http://localhost:11434", text: $host)
                .textFieldStyle(.roundedBorder)
                .focused($focused)
                .onTapGesture {
                    showsSuggestions = true
                    probe()
                }
                .onChange(of: host) { _, _ in showsSuggestions = focused }
                .onSubmit { showsSuggestions = false }
                .onChange(of: focused) { _, isFocused in
                    if !isFocused { showsSuggestions = false }
                }

            if showsSuggestions, !suggestions.isEmpty {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(suggestions, id: \.self) { suggestion in
                        Button {
                            host = suggestion
                            showsSuggestions = false
                        } label: {
                            HStack(spacing: 6) {
                                Image(systemName: "checkmark.circle.fill")
                                    .foregroundStyle(Color.green.opacity(0.8))
                                Text(suggestion)
                                Spacer(minLength: 0)
                            }
                            .font(
                                themeStore.uiFont(
                                    size: CGFloat(themeStore.settings.fontSize - 2),
                                    weight: .regular)
                            )
                            .padding(.horizontal, 8)
                            .padding(.vertical, 5)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(3)
                .background(
                    themeStore.surfaceFill(0.9),
                    in: RoundedRectangle(cornerRadius: 6, style: .continuous))
            }
        }
        .onAppear { probe() }
        .onDisappear { probeTask?.cancel() }
    }

    /// Keeps only the candidates that answered, so a suggestion is evidence a
    /// daemon is there rather than a guess the user has to verify themselves.
    private func probe() {
        probeTask?.cancel()
        probeTask = Task {
            var found: [String] = []
            for candidate in Self.candidates {
                if Task.isCancelled { return }
                let models = await OllamaProvider.listModels(host: candidate)
                if !models.isEmpty {
                    found.append(candidate)
                }
            }
            if Task.isCancelled { return }
            reachable = found
        }
    }
}
