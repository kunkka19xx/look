import SwiftUI

/// Ollama model chooser (control only, no label) that lists the models actually
/// installed on the local daemon (`GET /api/tags`), so the user picks instead of
/// typing a name that may not exist. Falls back to a free-text field when the
/// daemon is unreachable.
struct OllamaModelField: View {
    let host: String
    @Binding var model: String
    let themeStore: ThemeStore

    @State private var installed: [String] = []
    @State private var probeTask: Task<Void, Never>?

    var body: some View {
        Group {
            if installed.isEmpty {
                TextField("qwen3.5:4b", text: $model)
                    .textFieldStyle(.roundedBorder)
            } else {
                Menu {
                    ForEach(installed, id: \.self) { name in
                        Button {
                            model = name
                        } label: {
                            if name == model {
                                Label(name, systemImage: "checkmark")
                            } else {
                                Text(name)
                            }
                        }
                    }
                } label: {
                    Text(model.isEmpty ? "Select a model" : model)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .onAppear { refresh(debounced: false) }
        .onChange(of: host) { _, _ in refresh(debounced: true) }
        .onDisappear { probeTask?.cancel() }
    }

    /// One probe at a time, debounced while the host is being typed: a request
    /// per keystroke both hammers the daemon and can land out of order, showing
    /// a previous host's models. Cancelling the prior task guarantees the last
    /// edit wins.
    private func refresh(debounced: Bool) {
        probeTask?.cancel()
        let currentHost = host
        probeTask = Task {
            if debounced {
                try? await Task.sleep(nanoseconds: 400_000_000)
                if Task.isCancelled { return }
            }
            let models = await OllamaProvider.listModels(host: currentHost)
            // A superseded probe must not repaint the menu.
            if Task.isCancelled || currentHost != host { return }
            installed = models
        }
    }
}
